use crate::engine::Request;
use crate::i18n::tr as t;
use std::sync::mpsc::Sender;

pub struct Tray {
    pub tx: Sender<Request>,
    /// Muda quando o idioma muda, para o menu ser redesenhado.
    pub language: String,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        crate::desktop::APP_ID.into()
    }

    fn title(&self) -> String {
        "NZXT CAM".into()
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let size = 32;
        let argb = crate::gui::tray_icon_rgba(size as u32)
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[3], p[0], p[1], p[2]])
            .collect();
        vec![ksni::Icon {
            width: size,
            height: size,
            data: argb,
        }]
    }

    fn activate(&mut self, _: i32, _: i32) {
        let _ = self.tx.send(Request::Show);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: t("Abrir NZXT CAM"),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(Request::Show);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: t("Sair"),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(Request::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn(tx: Sender<Request>) -> Result<ksni::blocking::Handle<Tray>, ksni::Error> {
    use ksni::blocking::TrayMethods;
    Tray {
        tx,
        language: String::new(),
    }
    .spawn()
}
