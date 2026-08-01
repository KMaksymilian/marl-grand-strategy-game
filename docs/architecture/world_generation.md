# Dokumentacja Techniczna Mapy

Mapa jest głównym obszarem działań dla graczy. Definiuje ona dostępną przestrzeń do zabudowy wyznacza granicę możliwości działań poprzez system granic i prowincji oraz odzwierciedla ogólny stan symulacji w danym momencie. Mapa ma charakter dwuwymiarowy, określoną szerokość (**WIDTH**) oraz wysokość (**HEIGHT**).

**Standardowe wymiary mapy:**
**WIDTH = 2000**
**HEIGHT = 1500**

## 1. Komponenty geograficzne

### 1.1 Klasyfikacja wysokości terenu (Elevation)
Typ wyliczeniowy ***Elevation*** służy do określenia ukształtowania i wysokości danej komórki mapy Informacja o przydziale do konkretnej kategorii pochodzi ze znormalizowanej wartości
(w przedziale *0.0- 1.0*) z generatora szumu. Wartości te zostały dobrane empirycznie, aby zapewnić optymalny i realistyczny wygląd lądów. 

- **Ocean**: *0.0 – 0.2*
- **Lowland (Niziny)**: *0.2 – 0.4*
- **Plains (Równiny)**: *0.4 – 0.6*
- **Highland (Wyżyny)**: *0.6 – 0.8*
- **Mountain (Góry)**: *0.8 – 1.0*

### 1.2 Klasyfikacja wilgotności (Moisture)
Typ wyliczeniowy ***Moisture*** określa poziom wilgotności komórki mapy na podstawie znormalizowanej wartości (*0.0 - 1.0*) pochodzącej z dedykowanej mapy szumu. Przydział kategorii prezentuje się
następująco:

- **Dry (Suchy)**: *0.0 – 0.4*
- **Normal (Umiarkowany)**: *0.4 – 0.5*
- **Humid (Wilgotny)**: *0.5 – 0.6*
- **Wet (Mokry)**: *0.6 – 1.0*

### 1.3 Typ terenu (Biome)
Typ wyliczeniowy ***Biome*** definiuje ostateczny typ terenu dla danej komórki. Zależy on bezpośrednio od kombinacji ustalonych wcześniej wartości ***Elevation*** oraz ***Moisture***.

| **Elev. Moist** | **Dry** | **Normal** | **Humid** | **Wet**    |
|-----------------|---------|------------|-----------|------------|
| **Ocean**       | Ocean   | Ocean      | Ocean     | Ocean      |
| **Lowland**     | Desert  | Grassland  | Forest    | Rainforest |
| **Plains**      | Desert  | Grassland  | Forest    | Forest     |
| **Highland**    | Hills   | Hills      | Taiga     | Taiga      |
| **Mountain**    | Tundra  | Tundra     | Snow      | Snow       |

### Struktura Area
Struktura ***Area*** przechowuje podstawowe informacje geograficzne pojedynczej komórki. Zawiera dwie wartości typu zmiennoprzecinkowego *f32* z zakresu (*0.0 - 1.0*): wysokość terenu
(***elevation_val***) oraz wilgotność (***moisture_val***). Na podstawie tych parametrów metody struktury pozwalają wyliczyć odpowiednie wartości z typów wyliczeniowych.

## 2. Architektura struktur Świata i Mapy
W ogólności, dla zachowania wysokiej wydajności oraz ciągłości pamięci podręczne(cache-friendly), dane dwuwymiarowe mapy przetrzymywane są w płaskich, jednowymiarowych wektorach. Przejście z indeksowania dwuwymiarowego *(x, y)* na jednowymiarowe realizowane jest przy pomocy relacji: *idx = y * WIDTH + x*

```mermaid
classDiagram
    class World {
        +Map map
        +Vec~Province~ provinces
    }

    class Map {
        +usize width
        +usize height
        +Vec~Area~ terrain
        +Vec~usize~ territory
    }

    class Province {
        +usize id
        +Vec~Point~ territory
        +centroid() Point
    }

    class Area {
        +f32 elevation_val
        +f32 moisture_val
        +determine_biome() Biome
        +determine_elevation() Elevation
        +determine_moisture() Moisture
    }

    class Point {
        +usize x
        +usize y
    }

## 3. Modele Konfiguracyjne Generacji Mapy
Proces generacji proceduralnej jest sterowany przez zestaw konfiguracji, które pozwalają na dokładne dostrojenie kształtu i zachowania algorytmów na każdym etapie.

```mermaid
classDiagram
    class WorldDimensions {
        +usize width
        +usize height
    }

    class VoronoiConfig {
        +usize province_count
        +usize iteration_count
    }

    class NoiseConfig {
        +f64 elevation_scale
        +f64 moisture_scale
        +(f64, f64) min_max
    }

    class WarpConfig {
        +f64 warp_scale
        +f64 warp_intensity
    }

    class SeedConfig {
        +Option~u64~ points_seed_option
        +Option~u32~ borders_seed_option
        +Option~u32~ elevation_seed_option
        +Option~u32~ moisture_seed_option
    }

    class ElevationTuningFineConfig {
        +f64 distance_multiplyer
        +i32 dropoff_powi
        +f64 dropoff_multiplyer
        +f64 final_val_addition
    }

## 4. Generacja Proceduralna Danych Geograficznych
Generacja podstawowych danych oparta jest na mapach szumów z wykorzystaniem biblioteki *Noise*. W celu zapewnienia wysokiej wydajności, obliczenia niezależnych punktów wykonywane są współbieżnie z pomocą biblioteki *Rayon*.

### 4.1 Inicjalizacja i Siatka
Rozmiar mapy definiowany jest przez ***WorldDimensions.width*** oraz ***WorldDimenstions.height***. Na wstępie struktury takie jak ***terrain*** (komórki) oraz ***territory*** (przynależność do prowincji) zostają wypełnione wartościami domyślnymi (wartości zerowe parametrów w klasie ***Area***).

### 4.2 Wysokość terenu i wilgotność
Dla każdej komórki mapy obrabiane są dwie warstwy wyliczane z użyciem współbieżnego iterowania:

- **Zaburzenie (Warping)**: Współrzędne mapowane są za pomocą zaburzeń konfigurowanych w 
***WarpConfig*** (korzystając z parametrów ***warp_scale*** i ***warp_intensity***) by złamać zbytnio sztuczną regularność układu *perlin-noise*. Do determinizmu używany jest seed 
***SeedConfig.borders_seed_option***.

- **Wysokość**: Wykorzystuje ***NoiseConfig.elevation_scale***, by generować łagodne formacje
skalne, oraz zbiór parametrów z ***ElevationTuningFineConfig***, który sztucznie formuje kształt wyspy (im bliżej krawędzi mapy, tym większy ***dropoff*** — spadek wartości powodujący tworzenie się oceanu na krawędziach). Detalowane jest to ziarnem z ***SeedConfig.elevation_seed_option***.

- **Wilgotność**: Skalowana jest przez ***NoiseConfig.moisture_scale*** z pomocą ***SeedConfig.moisture_seed_option***.

## 5. Generacja Prowincji
Prowincje pozwalają pogrupować setki tysięcy komórek w zarządialne byty terytorialne zachowujące realistyczne, kręte i poszarpane granice naturalne.

### 5.1 Inizjalizacja Punktów Startowych
Na mapę rzucana jest pula rozłącznych, losowych punktów startowych. Ilość tych punktów jest
zdefiniowana w ***VoronoiConfig.province_count***. Proces losowania jest całkowicie deterministyczny o ile podano wartość w ***SeedConfig.points_seed_option***.

### 5.2 Algorytm Voronoi i Relaksacja Lloyda
Główny silnik podziału terytorialnego opiera się o mocno rozbudowany *algorytm Voronoi*(diagram Woronoja). W klasycznej wersji algorytm ten dzieli płaszczyznę na podstawie najmniejszej odległości komórki mapy *(x, y)* od wcześniej wygenerowanych punktów startowych prowincji. W tym systemie proces jest modyfikowany o kilka kluczowych elementów odwołujących się bezpośrednio do struktur konfiguracyjnych:

- **Zaburzenie Przestrzeni (Warp Field)**: Kształt komórek przy obliczaniu dystansu nie jest idealnie euklidesowy. Współrzędne poddawane są zniekształceniu na podstawie wartości 
***WarpConfig.warp_scale*** i ***WarpConfig.warp_intensity***. Powoduje to naturalne falowanie granic pomiędzy regionami.

- **Iteracyjna Relaksacja Lloyda**: Proces podziału Voronoi jest powtarzany określoną w konfiguracji ilość razy, zdefiniowaną bezpośrednio w ***VoronoiConfig.iteration_count***. Na koniec każdej z iteracji wyliczany jest centroid (fizyczny środek ciężkości) dla wygenerowanej prowincji, który w kolejnym kroku staje się jej nowym punktem startowym. Proces ten harmonizuje przestrzenne rozmieszczenie prowincji zapewniając spójność przy jednoczesnym uwzględnieniu szumu granic.

### 5.3 Metody Końcowe
Ostatnim etapem formowania układu prowincji jest upewnienie się, że granice lądowo-wodne przebiegają bezbłędnie:

- Jeżeli prowincja posiada jakiekolwiek tereny zakwalifikowane jako poniżej poziomu morza
(***Elevation::OCEAN_LEVEL***), cała struktura prowincji jest transformowana w terytorium wodne, wyrównując w ten sposób mechanizmy symulacyjne.

- Zabieg *"Ocean Set"* wyrównuje parametry wysokości i wilgotności dla terytoriów wodnych — sztywno przypisującim najwyższą wilgotność (*moisture = 1.0*) oraz wyrównując wartość wysokości (*elevation = 0.0*).