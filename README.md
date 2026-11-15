METSÄVALUAATIO-GUI
===================

[![Rust](https://github.com/jussinokelainen/metsavaluaatio-gui/actions/workflows/rust.yml/badge.svg?branch=master)](https://github.com/jussinokelainen/metsavaluaatio-gui/actions/workflows/rust.yml)

[![Release](https://img.shields.io/github/v/release/jussinokelainen/metsavaluaatio-gui?sort=semver)](https://github.com/jussinokelainen/metsavaluaatio-gui/releases)
![Homebrew](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fgithub.com%2Fjussinokelainen%2Fhomebrew-metsavaluaatio%2Fraw%2Frefs%2Fheads%2Fmaster%2Fgui_version.json&query=%24.version&label=homebrew)

GUI versio Metsävaluaatio ohjelmasta.

CLI Versio: https://github.com/jussinokelainen/metsavaluaatio

Asennus
-------

#### Arch Linux

```bash
git clone git@github.com:jussinokelainen/metsavaluaatio-gui.git
cd metsavaluaatio-gui/pkgbuild/arch
makepkg -si
```

#### macOS (homebrew)

```bash
brew tap jussinokelainen/metsavaluaatio
brew install --cask metsavaluaatio-gui
```
HUOM! Asennus poistaa Metsavaluaatio.app tiedostosta com.apple.quarantine attribuutin.

#### Cargo

```bash
git clone ssh://git@gitlab.internal.nokehub.net:222/jussinokelainen/metsavaluaatio-gui.git
cd metsavaluaatio-gui
cargo install --path . --bin metsaval-gui
```

Muista lisätä `~/.cargo/bin` `$PATH`-muuttujaan esimerkiksi `~/.bashrc`
tiedostoon:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

##### Riippuvuudet

```bash
rust
cmake
sqlite
```

Ohjelman käyttäminen vaatii myös graafisen käyttöliittymän.

Käyttö
------

Ennen ensimmäistä arviota tarvitaan henkilökohtainen MML API-avain. Sen voi
syöttää sovelluksen vasemman paneelin **Lisäasetukset**-osiossa, jolloin avain
tallennetaan tiedostoon:

```
~/.config/metsavaluaatio/config.toml
```

Kun API-avain on olemassa:

1. Syötä kiinteistötunnus, esimerkiksi `123-45-6-789`
2. Jos kunnalle ei ole vielä tallennettua nimeä, syötä se kerran. Nimi
   muistetaan seuraavaa kertaa varten.
3. Valitse halutessasi valmis GeoPackage tai GeoPackagen latauskansio
   **Lisäasetukset**-osiossa.
4. Paina **Hae arvio**.
5. Valitse tulostekansio ja paina **Tallenna Excel + KML** jos haluat tallentaa tiedot.
6. Jos haluat avata selaimen heti, sen voi tehdä suoraan sovelluksesta **Avaa Google My Maps** napista

Kartan luominen
---------------

Selaimessa klikkaa **Create new map**. Vasemmalla olevassa ikkunassa
**Untitled layer** -tason alapuolelta klikkaa **import** -> **Upload** ja valitse
ohjelman luoma `.kml`-tiedosto.

Klikkaa lisätyn tason yläpuolella **Share**, nimeä kartta, valtuuta kaikki
linkin saaneet avaamaan kartta ja kopioi linkki alareunasta.

Ohjelman poistaminen
--------------------

Paketinhallinnan kautta, tai

Cargolla asennettuna:

```bash
cargo uninstall metsaval
```

Testit
------

Kirjaston ja CLI:n testit:

```bash
cargo test --no-default-features
```

GUI-binäärin testit:

```bash
cargo test --bin metsaval-gui
```

GPUI-käyttöliittymän vuorovaikutustestit (tekstikentät, näppäimet, leikepöytä):

```bash
cargo test --features gui-test-support --bin metsaval-gui
```

