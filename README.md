METSÄVALUAATIO-GUI
===================

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
HUOM! Asennus poistaa .app tiedostosta com.apple.quarantine attribuutin.

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

macOS:lla tarvitaan lisäksi Xcode Command Line Tools GPUI:n Metal-käännökseen.

Linuxissa graafinen sovellus tarvitsee lisäksi X11- tai Wayland-ympäristön,
`libxkbcommon`- ja `libxkbcommon-x11`-kirjastot, XCB- ja freetype-tuen,
fontconfig-pohjaisen fonttituen sekä Vulkan-ajurin. GPUI piirtää
`blade`-renderöijällä.

Käyttö
------

Ennen ensimmäistä arviota tarvitaan henkilökohtainen MML API-avain. Sen voi
syöttää sovelluksen vasemman paneelin **Lisäasetukset**-osiossa, jolloin avain
tallennetaan tiedostoon:

```
~/.config/metsavaluaatio/config.toml
```

Käynnistyksen jälkeen:

1. Syötä kiinteistötunnus, esimerkiksi `740-555-2-4`.
2. Jos kunnalle ei ole vielä tallennettua nimeä, syötä se kerran. Nimi
   muistetaan seuraavaa kertaa varten.
3. Valitse tarvittaessa valmis GeoPackage tai GeoPackagen latauskansio
   **Lisäasetukset**-osiossa.
4. Paina **Hae arvio**.
5. Tarkastele kuvio- ja puulajikohtaisia tuloksia sekä lokia.
6. Valitse tulostekansio ja paina **Tallenna Excel + KML**.

Graafisessa käyttöliittymässä voit:

- hakea arvion kiinteistötunnuksella
- syöttää ja tallentaa MML API-avaimen
- käyttää valmista GeoPackagea tai antaa latauskansion
- tarkastella kuviokohtaisia ja puulajikohtaisia tuloksia
- tallentaa Excel-yhteenvedon ja Google Maps -tason (KML) valittuun kansioon
- avata Google My Maps -sivun ja tulostekansion suoraan sovelluksesta

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

