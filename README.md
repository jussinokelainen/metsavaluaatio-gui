METSÄVALUAATIO-GUI
===================

[![Rust](https://github.com/jussinokelainen/metsavaluaatio-gui/actions/workflows/rust.yml/badge.svg?branch=master)](https://github.com/jussinokelainen/metsavaluaatio-gui/actions/workflows/rust.yml)

[![Release](https://img.shields.io/github/v/release/jussinokelainen/metsavaluaatio-gui?sort=semver)](https://github.com/jussinokelainen/metsavaluaatio-gui/releases)
![Homebrew](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fgithub.com%2Fjussinokelainen%2Fhomebrew-metsavaluaatio%2Fraw%2Frefs%2Fheads%2Fmaster%2Fgui_version.json&query=%24.version&label=homebrew)

GUI versio Metsävaluaatio ohjelmasta.

CLI Versio: https://github.com/jussinokelainen/metsavaluaatio

Core kirjasto: https://github.com/jussinokelainen/metsavaluaatio-core

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
git clone git@github.com:jussinokelainen/metsavaluaatio-gui.git
cd metsavaluaatio-gui
cargo install --path .
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
syöttää sovelluksen oikean yläkulman rataskuvakkeesta avautuvassa
**Asetukset**-näkymässä.

Kun API-avain on olemassa:

1. Syötä kiinteistötunnus, esimerkiksi `123-45-6-789`
2. Jos kunnalle ei ole vielä tallennettua nimeä, syötä se kerran. Nimi
   muistetaan seuraavaa kertaa varten.
3. Paina **Hae arvio**.
4. Valitse tulostekansio ja paina **Tallenna Excel + KML** jos haluat tallentaa tiedot.
5. Jos haluat avata selaimen heti, sen voi tehdä suoraan sovelluksesta **Avaa Google My Maps** napista

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
