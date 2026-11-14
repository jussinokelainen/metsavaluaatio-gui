METSÄVALUAATIO-GUI
===================

Metsäomaisuuden arviointiin tarkoitettu GPUI-pohjainen työpöytäsovellus.
Ohjelma hakee kiinteistön rajat, metsävarakuviot ja puustotiedot sekä laskee
laskennallisen arvon. Tulokset voi tallentaa Excel-yhteenvetona ja Google Maps
-tasona (KML).

Tämä repo on graafisen käyttöliittymän haara. Paketit asentavat vain
`metsaval-gui`-sovelluksen; repossa mukana oleva jaettu laskentakirjasto ja
alkuperäinen komentorivityökalu eivät kuulu asennettaviin paketteihin.

Asennus
-------

#### Arch Linux

```bash
git clone ssh://git@gitlab.internal.nokehub.net:222/jussinokelainen/metsavaluaatio-gui.git
cd metsavaluaatio-gui/pkgbuild/arch
makepkg -si
```

Uusinta git-committia seuraava paketti:

```bash
makepkg -si -p PKGBUILD-git
```

#### macOS (homebrew)

```bash
brew tap jussinokelainen/metsavaluaatio
brew install metsavaluaatio-gui
```

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

Ei-git-paketit (`PKGBUILD` ja Homebrew-formula) rakennetaan versiontagista.
Tagin tulee vastata `Cargo.toml`-tiedoston versiota, esimerkiksi `0.1.0`.
Uusinta committia seuraava Arch-paketti on `PKGBUILD-git`.

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

Paketit listaavat tarvittavat järjestelmäriippuvuudet automaattisesti.

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

Paketinhallinnan kautta:

```bash
sudo pacman -R metsavaluaatio-gui      # tai metsavaluaatio-gui-git
brew uninstall metsavaluaatio-gui
```

Cargolla asennettuna:

```bash
cargo uninstall metsaval
```
