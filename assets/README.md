# Ícones e logo do NZXT CAM

Os ícones originais foram incluídos por pedido expresso do usuário em 13/11/2026
para a bandeja, a integração com o sistema e a logo do cabeçalho. São recursos da NZXT e não
estão cobertos pela licença MIT do código deste projeto independente.

| Arquivo | Origem no NZXT CAM Windows 4.76.8 | Uso |
| --- | --- | --- |
| `nzxt-cam.png` | `resources/static/app_icons/tray.png`, cópia intacta, 512 × 512 | Lançador, dock e janela |
| `nzxt-cam-tray.png` | Frame 32 × 32 de `resources/static/app_icons/tray.ico`, convertido para PNG | Bandeja, pixmap ARGB |
| `nzxt-cam-wordmark.svg` | `assets/originals/logo nzxt cam.svg` dentro de `resources/app.asar`, cópia intacta | Fonte vetorial da logo do cabeçalho |
| `nzxt-cam-wordmark.png` | Rasterização do SVG acima, 872 × 144, branco sobre transparência | Cabeçalho, 124 px de largura e proporção original |

A referência completa permanece em `package/`, fora do Git. O instalador publica
o ícone do aplicativo no tema hicolor; a bandeja embute seu pixmap para não
depender da instalação de um tema.

A rasterização aplica preenchimento branco ao desenho, como no tema escuro,
sem depender das fontes da NZXT. O PNG é embutido no binário e carregado uma vez
pela GUI; não exige navegador nem biblioteca SVG durante a execução.
