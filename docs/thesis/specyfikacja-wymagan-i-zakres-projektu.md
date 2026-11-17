# Etap 1: Specyfikacja wymagań i zakres projektu

**Zespołowy projekt inżynierski — Wydział Matematyki i Nauk Informacyjnych Politechniki Warszawskiej**
**Rok akademicki: 2026/2027**

## 1. Metryka dokumentu i historia zmian

| Wersja | Data | Autor | Opis zmiany |
| --- | --- | --- | --- |
| 0.1 | 19.10.2026 | Maksymilian Majewski, Łukasz Przybylski                | Projekt specyfikacji |

- **Tytuł projektu:** MARL Grand Strategy Simulation — wieloagentowy system symulacji procesów gospodarczych i geopolitycznych w środowisku gry strategicznej.
- **Autorzy:** Maksymilian Majewski, Łukasz Przybylski.
- **Promotor:** mgr. Szymon Zyguła.
- **Uczelnia i wydział:** Politechnika Warszawska, Wydział Matematyki i Nauk Informacyjnych.
- **Kierunek:** Informatyka i Systemy Informacyjne.
- **Podstawa analizy:** repozytorium `marl-grand-strategy`, rewizja `673048c`, stan odczytany 19.10.2026.

Dokument określa wymagania wobec docelowego rezultatu projektu.

## 2. Sformułowanie problemu inżynierskiego i badawczego

> Celem projektu jest zaprojektowanie, zaimplementowanie i eksperymentalna ocena turowego środowiska symulacyjnego gry typu grand strategy, w którym wiele autonomicznych osad zarządzanych przez agentów podejmuje decyzje o produkcji, rozwoju i handlu.

Pytanie badawcze brzmi: czy wyuczone strategie agentów umożliwiają skuteczne zarządzanie współzależnymi gospodarkami osad w nowych scenariuszach, w porównaniu z prostymi regułami decyzyjnymi oraz istniejącymi modelami sztucznej inteligencji?

## 3. Zakres projektu: MVP i dalszy rozwój

### 3.1. Minimalny zakres funkcjonalny — MVP

Zakres MVP wynika z kategorii MUST w priorytetach MoSCoW. Obejmuje kompletny scenariusz gospodarczy wielu osad z agentami MARL.

1. **Środowisko przestrzenne:** generowanie mapy z terenem i prowincjami, lokalizacja osad i budynków oraz wyznaczanie tras transportowych z uwzględnieniem kosztu przemieszczania.
2. **Turowa symulacja wielu osad:** wspólna pętla tur obejmująca obserwacje agentów, przyjęcie działań, rozliczenie gospodarki oraz transport.
3. **Gospodarka lokalna:** magazynowanie materiałów, potrzeby mieszkańców, produkcja, budowa i ulepszanie budynków, niedobory.
4. **Handel i logistyka:** składanie ofert kupna i sprzedaży, rezerwacja środków i towarów, rozliczenie transakcji oraz dostawa z opóźnieniem wynikającym z trasy.
5. **Podejmowanie decyzji przez AI:** zdefiniowany zbiór obserwacji, akcji i nagród, co najmniej jedna wytrenowana polityka MARL oraz możliwość oceny bez aktualizowania parametrów.
6. **Badanie jakości strategii:** porównanie polityki wyuczonej z innymi rozwiązaniami obecnymi na rynku.


### 3.2. Rozszerzenia poza MVP

| Priorytet MoSCoW | Funkcje | Warunek realizacji |
| --- | --- | --- |
| SHOULD | Państwa grupujące osady i wyznaczające cele strategiczne; warstwa HRL; rekrutacja, ruch i walka jednostek; dyplomacja, wojny, sojusz. | Po uzyskaniu stabilnego MVP i dostępności czasu na implementację oraz ocenę. |
| COULD | Drzewo technologiczne, migracja ludności, interfejs pozwalający graczowi kierować państwem. | Rozszerzenia opcjonalne; przejęcie państwa przez gracza wymaga odpowiedniego interfejsu. |
| WON'T w bieżącym zakresie | Model kultury i podział ludności na klasy społeczne. | Nie są wymagane do realizacji przyjętego problemu. |


### 3.3. Stan wyjściowy implementacji

| Obszar | Stan na dzień opracowania | Prace niezbędne do MVP |
| --- | --- | --- |
| Mapa i prezentacja | Generator terenu i prowincji, podgląd, kamera i wizualizacja wyszukiwania ścieżek. | Połączenie mapy z osadami i scenariuszami eksperymentalnymi. |
| Gospodarka osady | Magazyny, potrzeby, przydział zasobów, produkcja, projekty i raporty lokalnej tury. | Integracja wielu osad, sterowanie przez agentów i testy całego przebiegu symulacji. |
| Produktywność budynków | Wpływ kosztu trasy A* na produkcję. | Weryfikacja działania w scenariuszach wieloosadowych. |
| Handel i karawany | Prototyp ofert, rezerwacji, dwustronnego dopasowania i ruchu po trasie. | Publiczne komendy handlu, globalna kolejność faz oraz obsługa zakończenia transportu. |
| Pętla całego świata | Aplikacja uruchamia generowanie i prezentację mapy. | Harmonogram tur gospodarki, decyzji i transportu; tryb bez okna. |
| AI i eksperymenty | Brak implementacji modeli i treningu. | Interfejs środowiska, polityki heurystyczne, MARL, trening, ocena i eksport wyników. |
| Państwa i HRL | Architektura planowana. | Poza MVP. |



## 4. Wymagania funkcjonalne

Role „badacz” i „obserwator” oznaczają osoby uruchamiające lub analizujące symulację. „Agent osady” oznacza program podejmujący decyzje za pojedynczą osadę.

### Jednostki surowców

W całym systemie ilości surowców podaje się w tych samych jednostkach całkowitych. Ta sama wartość obowiązuje w tabelach scenariuszy, danych API i obliczeniach symulacji. Reguła obejmuje zapasy, pojemność magazynu, koszty budowy i ulepszeń, wejścia i wyjścia receptur, stawki zużycia na mieszkańca oraz limity handlu i transportu.

Receptura zużywająca 3 jednostki ryb i wytwarzająca 3 jednostki wędzonych ryb pobiera 3 z zapasu ryb i wytwarza 3 jednostki produktu przy pełnej wydajności. Stawka 1 jednostki żywności na mieszkańca na turę daje zapotrzebowanie 120 jednostek dla 120 mieszkańców. Limit przewozu 400 jednostek oznacza 400 jednostek towaru także w symulacji.

Ludność liczy się w osobach, a walutę w jednostkach złota. Cena jednostkowa jest podawana w złocie za jedną jednostkę surowca.

### US-01: Przygotowanie świata i scenariusza

**Jako badacz** chcę utworzyć świat z określonymi parametrami, ziarnami losowości i stanem początkowym osad, **aby** porównywać strategie w kontrolowanych warunkach.

- **Mając:** poprawną konfigurację mapy, co najmniej dwie osady oraz ich początkowe zasoby i budynki.
- **Gdy:** uruchomiona zostanie inicjalizacja scenariusza.
- **Wtedy:** powstaje świat z osadami o unikalnych identyfikatorach i prawidłowych pozycjach; konfiguracja oraz ziarna są zapisywane w metadanych uruchomienia.
- **Scenariusz błędny:** zerowe wymiary mapy, niemożliwa liczba prowincji lub pozycja osady poza mapą powodują czytelny błąd walidacji przed rozpoczęciem symulacji.

### US-02: Wykonanie wspólnej tury

**Jako badacz** chcę wykonywać kolejne globalne tury, **aby** obserwować skutki decyzji wszystkich osad we wspólnym świecie.

- **Mając:** zainicjalizowany scenariusz i zestaw działań osad dla tury `t`.
- **Gdy:** wykonywany jest jeden krok środowiska.
- **Wtedy:** każda osada rozlicza gospodarkę, transport jest przesuwany zgodnie z ustalonym harmonogramem i powstają raporty oraz obserwacje dla kolejnego kroku.
- **Scenariusz bezczynności:** brak decyzji osady jest zastępowany zdefiniowaną akcją bezczynności i nie zatrzymuje kroku.

### US-03: Sterowanie produkcją i magazynem

**Jako agent osady** chcę ustalać aktywność i obciążenie budynków oraz kolejność przyjmowania materiałów do magazynu, **aby** reagować na potrzeby mieszkańców i ograniczone zasoby.

- **Mając:** istniejący budynek i dostępne zasoby osady.
- **Gdy:** agent ustawia dopuszczalne obciążenie i wykonywana jest tura.
- **Wtedy:** zapotrzebowanie i produkcja uwzględniają obciążenie, faktycznie przyznane materiały, pracę i produktywność lokalizacji; nie powstają ujemne zapasy ani przekroczenie pojemności magazynu.
- **Wtedy:** raport zawiera ocenę zaspokojenia potrzeb, zużycie, produkcję i jawne straty; praca jest odnawiana według liczby mieszkańców.
- **Scenariusz błędny:** nieistniejący identyfikator budynku, niedozwolone obciążenie lub niepoprawna lista materiałów są odrzucane bez zmiany stanu.

### US-04: Rozbudowa osady

**Jako agent osady** chcę zlecać budowę i ulepszanie budynków, **aby** zwiększać przyszłe możliwości gospodarki.

- **Mając:** poprawną definicję budynku, pozycję, unikalny identyfikator projektu i koszt wynikający z konfiguracji scenariusza.
- **Gdy:** agent składa zlecenie budowy lub ulepszenia.
- **Wtedy:** system rejestruje projekt, rozlicza dostarczane materiały i upływ tur, a po spełnieniu obu warunków zgłasza ukończenie w raporcie.
- **Wtedy:** budynek ukończony po fazie produkcji może produkować najwcześniej w kolejnej turze po ustawieniu jego aktywności i obciążenia.
- **Scenariusz błędny:** nieistniejący cel ulepszenia, niepoprawna definicja lub zduplikowany aktywny identyfikator projektu powodują odrzucenie zlecenia bez pobrania zasobów.

### US-05: Wymiana i dostarczenie towarów

**Jako agent osady** chcę wymieniać nadwyżki na brakujące dobra i organizować transport, **aby** korzystać ze specjalizacji gospodarczej innych osad.

- **Mając:** sprzedawcę z 10 jednostkami drewna, kupującego z 40 jednostkami waluty, ofertę sprzedaży po minimum 3 i zakupu po maksimum 5 jednostek waluty oraz istniejącą trasę.
- **Gdy:** system dopasuje oferty i przeprowadzi transakcję w przewidzianej fazie handlu.
- **Wtedy:** cena jednostkowa wynosi 4, transakcja obejmuje 10 jednostek drewna i 40 jednostek waluty, a zarezerwowane dobra i środki nie mogą zostać ponownie wydane.
- **Wtedy:** towar staje się dostępny kupującemu po dostawie zgodnie z harmonogramem transportu; waluta i towar mają w każdej fazie jednoznacznie określonego właściciela.
- **Scenariusze graniczne:** brak zgodności cen nie tworzy transakcji; brak trasy nie uruchamia transportu i nie pozostawia utraconej rezerwacji; nadmiar dostawy ponad pojemność magazynu jest jawnie raportowany zgodnie z przyjętą regułą strat.

### US-06: Trening i uruchomienie polityki MARL

**Jako badacz** chcę trenować agentów i zapisywać model, **aby** później oceniać wyuczone zachowanie w nowych światach.

- **Mając:** działający interfejs środowiska oraz konfigurację algorytmu, modelu, ziaren i budżetu treningu.
- **Gdy:** uruchomiony zostanie trening z udziałem co najmniej dwóch osad.
- **Wtedy:** powstają zapis modelu, konfiguracja, przebieg metryk i liczba wykonanych kroków środowiska.
- **Gdy:** model zostaje wczytany w trybie oceny.
- **Wtedy:** podejmuje decyzje bez aktualizacji parametrów, a powtórne wczytanie tego samego zapisu zachowuje parametry i ustawienia przetwarzania obserwacji.
- **Wtedy:** dokumentacja określa, czy osady współdzielą model, jakie informacje są dostępne w treningu oraz jakie w wykonaniu.

### US-07: Porównanie strategii

**Jako badacz** chcę porównać agentów wyuczonych z innymi rozwiązaniami, **aby** ocenić skuteczność uczenia w kontrolowanych warunkach.

- **Mając:** zamrożone modele z co najmniej 3 niezależnych treningów i co najmniej 2 inne rozwiązania.
- **Gdy:** każda strategia jest oceniana na tym samym zestawie co najmniej 20 ziaren testowych niewykorzystanych do treningu ani strojenia, z horyzontem 500 tur.
- **Wtedy:** raport przedstawia średnie wyniki, rozrzut i liczbę obserwacji dla zaspokojenia potrzeb, tur z niedoborami, produkcji, strat oraz wolumenu handlu; osobno pokazuje wynik nagrody.

### US-08: Uruchomienie bez renderowania i eksport wyników

**Jako badacz** chcę wykonywać serie eksperymentów bez okna i zapisywać ich wyniki, **aby** automatyzować ocenę i tworzyć wykresy do pracy.

- **Mając:** konfigurację scenariusza, strategię i katalog wynikowy.
- **Gdy:** uruchomiona zostaje seria w trybie bez renderowania.
- **Wtedy:** nie jest tworzone okno ; wyniki zawierają metryki w CSV oraz metadane w formacie maszynowym, np. JSON.
- **Wtedy:** zapis obejmuje co najmniej rewizję kodu, konfigurację, ziarna, identyfikator modelu lub heurystyki, horyzont, wersje zależności i informację o sprzęcie.
- **Scenariusz błędny:** brak modelu lub niezgodna konfiguracja kończą uruchomienie czytelnym komunikatem i nie oznaczają eksperymentu jako ukończonego.

## 5. Mierzalne wymagania niefunkcjonalne

Referencyjne środowisko pomiarowe: komputer z co najmniej 4 fizycznymi rdzeniami CPU i 16 GB RAM, kompilacja Rust `release`, lokalny dysk SSD, bez renderowania.

Scenariusz referencyjny **B1**: mapa 256 × 256 pól, 64 prowincje, 50 osad po 10 budynków, 3 materiały, co najwyżej 2 aktywne projekty na osadę i 100 aktywnych karawan. Obiekty są rozmieszczane według zapisanej konfiguracji. Wydajność silnika mierzy się z heurystyką, bez treningu i bez czasu generowania świata; koszt inferencji modelu mierzy się oddzielnie. B1 służy do benchmarków i nie zmienia domyślnego rozmiaru mapy aplikacji.

| ID | Kategoria | Docelowa miara | Weryfikacja |
| --- | --- | --- | --- |
| NFR-01 | Czas tury | p95 czasu globalnej tury B1 ≤ 100 ms po 100 turach rozgrzewki, na kolejnych 1000 turach, w 5 uruchomieniach. | Benchmark z pomiarem monotonicznym; p95 i mediana raportowane osobno dla każdego uruchomienia. |
| NFR-02 | Przepustowość | W B1 co najmniej 10 pełnych tur świata/s jako średnia w każdym z 5 uruchomień; krok obejmuje ekonomię, handel i ruch. | Łączny czas tych samych 1000 tur; dodatkowo czasy poszczególnych faz. |
| NFR-03 | Pamięć i wykonanie modelu | Szczytowa pamięć procesu silnika w B1 ≤ 2 GiB; uruchomienie oceny zapisanej polityki dla 10 osad jest możliwe na CPU bez GPU. | Pomiar pamięci przez narzędzie systemowe oraz test oceny na CPU. Wymagania sprzętowe treningu są podane osobno. |
| NFR-04 | Odtwarzalność wykonania | 3 powtórzenia 500 tur z tymi samymi ziarnami, konfiguracją i deterministyczną heurystyką dają identyczny kanoniczny stan końcowy i metryki na tej samej platformie i wersji kodu. | Porównanie stanu z uporządkowanymi kolekcjami oraz plików metryk. Nie obejmuje pikseli podglądu ani identyczności treningu GPU. |
| NFR-05 | Poprawność rozliczeń | W 10 scenariuszach po 1000 tur: 0 niewyjaśnionych rozbieżności bilansu materiałów i waluty, 0 podwójnych wydań i 0 przekroczeń pojemności magazynu. | Testy integracyjne z bilansem magazynów, rezerwacji, ładunków, produkcji, zużycia i jawnych strat. Praca jest odnawianą zdolnością i nie podlega bilansowi materiałowemu. |
| NFR-06 | Odporność na błędne dane | Każdy przypadek z utrzymywanego zestawu co najmniej 20 niepoprawnych konfiguracji i działań jest odrzucany bez niekontrolowanego przerwania i częściowej mutacji stanu; zestaw obejmuje granice liczb, brak tras i niepoprawne ID. | Testy negatywne i porównanie stanu przed odrzuconą operacją i po niej. |
| NFR-07 | Odtwarzalność instalacji | Na czystym środowisku po instalacji udokumentowanych narzędzi wykonanie opisanej procedury pozwala uruchomić testy Rust, przykładową symulację bez okna i ocenę dostarczonego modelu. Wszystkie 3 kroki kończą się powodzeniem. | Próba instalacji na Windows i Linux, z wersjami Rust/Python i plikami ustalającymi zależności; bez limitu czasu pobierania i kompilacji. |
| NFR-08 | Kontrola jakości | Przed odbiorem przechodzą formatowanie, Clippy z ostrzeżeniami jako błędami, testy workspace oraz testy interfejsu Python i scenariusza integracyjnego; brak niezaliczonych obowiązkowych testów. | `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` oraz docelowe polecenie testów Python zapisane w instrukcji. |
| NFR-09 | Kompletność eksperymentów | 100% wyników użytych w porównaniu US-08 ma konfigurację, ziarna, identyfikator strategii, rewizję kodu i wersje zależności; model i raport są powiązane identyfikatorem uruchomienia. | Automatyczna walidacja metadanych przed wygenerowaniem tabel porównawczych. |


## 6. Słownik pojęć

| Pojęcie | Definicja w projekcie |
| --- | --- |
| Grand strategy | Gatunek gier strategicznych obejmujących zarządzanie gospodarką i relacjami politycznymi w dużej skali; w MVP realizowana jest warstwa gospodarcza. |
| Osada | Jednostka gospodarcza posiadająca ludność, zapasy, budynki i projekty, sterowana przez agenta lub heurystykę. |
| Agent | Program wybierający działania na podstawie dostępnych obserwacji i realizowanego celu. |
| RL | Uczenie ze wzmocnieniem: uczenie sposobu działania na podstawie interakcji ze środowiskiem i uzyskiwanych nagród. |
| MARL | Wieloagentowe uczenie ze wzmocnieniem; decyzje wielu agentów wpływają na wspólne środowisko i wyniki innych agentów. |
| HRL | Hierarchiczne uczenie ze wzmocnieniem: podział decyzji na cele wyższego poziomu i działania niższego poziomu; planowane rozszerzenie państw. |
| Polityka | Funkcja lub model określający wybór działania dla danej obserwacji. Wspólna polityka może sterować wieloma osadami. |
| Obserwacja | Informacje o stanie świata udostępniane konkretnemu agentowi; nie musi obejmować całego świata. |
| Akcja | Dopuszczalna decyzja agenta, np. zmiana obciążenia budynku, rozpoczęcie budowy lub oferta handlowa. |
| Nagroda | Liczbowa ocena skutków działania wykorzystywana do uczenia; jej wzór jest elementem konfiguracji eksperymentu. |
| Epizod | Przebieg symulacji od inicjalizacji do warunku zakończenia lub ustalonego limitu tur. |
| Heurystyka | Jawne reguły decyzyjne niewymagające treningu; punkt odniesienia w ocenie modelu. |
|  |
| Ziarno losowości | Wartość inicjująca generator pseudolosowy; wraz z konfiguracją i wersją kodu umożliwia kontrolowanie powtórzeń. |
| Globalna tura | Jeden krok całego świata obejmujący decyzje, gospodarkę i transport według określonej kolejności faz. |
| Karawana | Jednostka transportu przypisana do trasy i uczestników wymiany, przenosząca zasoby związane z handlem. |
| A* | Algorytm wyszukiwania trasy wykorzystujący koszt dotychczasowej drogi i heurystyczne oszacowanie kosztu pozostałego. |
 |
| MVP | Minimalny uzgodniony rezultat obejmujący wszystkie funkcje obowiązkowe i ich kryteria odbioru. |
|  |

## 7. Wstępny rejestr ryzyk i podział odpowiedzialności

### 7.1. Ryzyka

Oceny są wstępne: N — niskie, Ś — średnie, W — wysokie. Właściciel oznacza rolę odpowiedzialną za monitorowanie ryzyka; przypisanie osób wymaga ustalenia przez zespół.

| ID | Ryzyko | Prawdopodobieństwo / wpływ | Ograniczenie i plan awaryjny | Właściciel |
| --- | --- | --- | --- | --- |
| R-01 | Rozszerzenie projektu o państwa, wojsko i dyplomację opóźni część gospodarczą. | W / W | Najpierw odbiór pionowego scenariusza produkcja–handel–AI; rozszerzenia realizowane po MVP. | Koordynacja projektu |
| R-02 | Model będzie wykorzystywał błędy bilansu lub niepełne rozliczenia handlu. | W / W | Testy zachowania zasobów przed treningiem; pełna ewidencja rezerwacji, transportu i strat; w razie problemów trening wstrzymany do poprawy silnika. | Silnik i testy |
| R-03 | Uczenie będzie niestabilne albo nie zapewni przewagi nad heurystykami. | W / W | Wcześnie zbudować heurystyki, prosty scenariusz i monitoring; zmniejszać przestrzeń akcji i złożoność scenariusza, dokumentując zmiany. Wynik bez przewagi podlega analizie. | AI i eksperymenty |
| R-04 | Koszt wielu wywołań Rust–Python ograniczy trening. | Ś / W | Mierzyć kroki/s, grupować decyzje osad i unikać przesyłania całej mapy przy każdym kroku; wybrać sposób integracji po małym prototypie. | Integracja |
| R-05 | Wyznaczanie tras i generacja dużych map zdominują czas wykonania. | Ś / W | Profilować B1, rozdzielić inicjalizację od czasu tury i badać ponowne użycie tras; mniejsze mapy treningowe przy zachowaniu mechanik. | Środowisko i wydajność |
| R-06 | Nagroda zachęci do produkcji bez zaspokajania potrzeb lub do nieuzasadnionego gromadzenia zasobów. | Ś / W | Analizować wskaźniki gospodarcze oddzielnie od nagrody, sprawdzać skrajne strategie i wersjonować funkcję nagrody. | AI i eksperymenty |
| R-07 | Niepowtarzalność wyników albo przeciek scenariuszy testowych do treningu uniemożliwią rzetelne porównanie. | Ś / W | Oddzielne zbiory ziaren treningowych, walidacyjnych i testowych; zamrożenie protokołu oceny; zapis konfiguracji i modeli. | Eksperymenty i testy |
| R-08 | Brak wystarczającego czasu lub sprzętu na trening. | Ś / W | Wczesny pomiar kosztu krótkiego treningu, określony budżet kroków i sprzętu, mniejszy model i scenariusze; zachować wymagane niezależne powtórzenia. | Koordynacja i AI |

### 7.2. Podział odpowiedzialności

Poniższa tabela opisuje obszary pracy, nie rzeczywisty skład zespołu. Jedna osoba może pełnić kilka ról. Dla każdego obszaru zespół przypisuje jednego właściciela i osobę dokonującą przeglądu.

| Obszar | Odpowiedzialność i rezultat | Osoba |
| --- | --- | --- |
| Koordynacja i wymagania | Zakres, harmonogram, uzgodnienia z promotorem, aktualizacja specyfikacji i rejestru ryzyk. | Do przypisania |
| Silnik gospodarczy | Reguły ekonomii, komendy osad, bilans materiałów i waluty, budowa i produkcja. | Do przypisania |
| Świat i logistyka | Mapa, ścieżki, karawany, globalna pętla tur i pomiary wydajności. | Do przypisania |
| Integracja Rust–Python | Interfejs obserwacji i akcji, uruchamianie bez okna i zgodność danych. | Do przypisania |
| AI i eksperymenty | Heurystyki, model, nagroda, trening, protokół porównania i analiza wyników. | Do przypisania |
| Testy i dokumentacja | Testy integracyjne, CI, odtwarzalność uruchomień, instrukcje i kompletność artefaktów. | Do przypisania |



## 8. Formalna akceptacja promotora

- [ ] Promotor zapoznał się ze specyfikacją i zatwierdził zakres MVP.
- [ ] Uzgodniono kryteria odbioru oraz docelowe progi wymagań niefunkcjonalnych.

**Podpis promotora:** ........................................................ **Data:** ................................



###
