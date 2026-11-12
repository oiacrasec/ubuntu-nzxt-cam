use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Arc, RwLock},
};

/// Catálogos embutidos. PT-BR é a língua-fonte: a chave é o próprio texto.
const BUILTIN: &[(&str, &str)] = &[("en-US", include_str!("../locales/en-US.json"))];
const MAX_CATALOG: u64 = 512 * 1024;

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub tag: String,
    entries: BTreeMap<String, String>,
}

static ACTIVE: RwLock<Option<Arc<Catalog>>> = RwLock::new(None);

pub fn valid_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let language = parts.next().unwrap_or_default();
    (2..=3).contains(&language.len())
        && language.bytes().all(|b| b.is_ascii_lowercase())
        && parts.all(|p| (2..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn placeholders(text: &str) -> Vec<&str> {
    let mut out: Vec<&str> = text
        .match_indices('{')
        .filter_map(|(start, _)| {
            let end = text[start..].find('}')?;
            Some(&text[start..=start + end])
        })
        .collect();
    out.sort_unstable();
    out
}

fn parse(tag: &str, text: &str) -> Result<Catalog> {
    let entries: BTreeMap<String, String> =
        serde_json::from_str(text).with_context(|| format!("Catálogo {tag} inválido"))?;
    for (key, value) in &entries {
        if placeholders(key) != placeholders(value) {
            bail!("Catálogo {tag}: marcadores diferentes em \"{key}\"");
        }
    }
    Ok(Catalog {
        tag: tag.into(),
        entries,
    })
}

/// Carrega o catálogo do idioma: primeiro o diretório do usuário, depois o embutido.
pub fn load(tag: &str, config_path: &Path) -> Result<Catalog> {
    if !valid_tag(tag) {
        bail!("Código de idioma inválido");
    }
    if tag == "pt-BR" {
        return Ok(Catalog {
            tag: tag.into(),
            entries: BTreeMap::new(),
        });
    }
    if let Some(dir) = config_path.parent() {
        let path = dir.join("locales").join(format!("{tag}.json"));
        if let Ok(meta) = fs::metadata(&path) {
            if meta.len() > MAX_CATALOG {
                bail!("Catálogo {tag} excede o limite de tamanho");
            }
            return parse(tag, &fs::read_to_string(&path)?);
        }
    }
    let Some((_, text)) = BUILTIN.iter().find(|(t, _)| *t == tag) else {
        bail!("Idioma {tag} não encontrado");
    };
    parse(tag, text)
}

/// Idiomas disponíveis: PT-BR, embutidos e catálogos do usuário.
pub fn available(config_path: &Path) -> Vec<String> {
    let mut tags: Vec<String> = std::iter::once("pt-BR".to_owned())
        .chain(BUILTIN.iter().map(|(t, _)| (*t).to_owned()))
        .collect();
    if let Some(dir) = config_path.parent()
        && let Ok(entries) = fs::read_dir(dir.join("locales"))
    {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(tag) = name.strip_suffix(".json")
                && valid_tag(tag)
            {
                tags.push(tag.to_owned());
            }
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

pub fn activate(catalog: Catalog) {
    *ACTIVE.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(catalog));
}

pub fn tr(text: &str) -> String {
    ACTIVE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|c| c.entries.get(text).cloned())
        .unwrap_or_else(|| text.to_owned())
}

/// Tradução com substituição de `{nome}`.
pub fn trf(text: &str, values: &[(&str, &str)]) -> String {
    let mut out = tr(text);
    for (name, value) in values {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_language_tags() {
        for tag in ["pt-BR", "en-US", "de", "zh-Hant"] {
            assert!(valid_tag(tag), "{tag}");
        }
        for tag in ["", "P", "pt_BR", "../x", "pt-", "português"] {
            assert!(!valid_tag(tag), "{tag}");
        }
    }

    #[test]
    fn builtin_catalogs_parse_with_matching_placeholders() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        let catalog = load("en-US", &config).unwrap();
        assert!(!catalog.entries.is_empty());
        assert!(load("xx-YY", &config).is_err());
        assert_eq!(available(&config), vec!["en-US", "pt-BR"]);
    }

    #[test]
    fn rejects_placeholder_mismatch_in_user_catalog() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("locales")).unwrap();
        fs::write(
            dir.path().join("locales/es-ES.json"),
            r#"{"Canal {nome}": "Canal"}"#,
        )
        .unwrap();
        assert!(load("es-ES", &dir.path().join("config.json")).is_err());
        assert!(available(&dir.path().join("config.json")).contains(&"es-ES".to_owned()));
    }
}
