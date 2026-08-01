# Dokumentacja techniczna Kamery

Kamera jest strukturą służącą do obserwacji postępu symulacji przeprowadzanej na mapie.
Wszelka jej obsługa oraz obserwacja świata jest umożliwiona dzięki bibliotece *macroquad*

## 1. Inicjalizacja

### Struktura MapView
Struktura ***MapView*** na podstawie utworzonej mapy generuje prostą teksturę 2D (***Texture2D***). 

- Dla każdej komórki mapy wyliczany jest odpowiedni kolor na podstawie biomu, który się tam znajduje. Każdy z nich dostaje unikalny kolor, dzięki czemu istnieje widoczne rozróżnienie między nimi.
- Ponadto dodany jest prosty shader, który na podstawie różnicy wysokości dodaje bądź odejmuje poziom natężenia światła poprzez zmianę składowych palety koloru *RGB*.
- Dodane są również krawędzie między prowincjami jak i na obrzeżach mapy.

### Struktura MapCamera
Struktura ***MapCamera*** posiada pola *speed* oraz *camera*, gdzie ta druga jest wbudowaną strukturą ***Camera2D*** z biblioteki *macroquad*. Ten obiekt odpowiada za główną obsługę kamery tj. przybliżanie (*zoom*) oraz przesuwanie punktu widzenia kamery (*target*) za pomocą przycisków *W,S,A,D*. Pole *speed* reguluje szybkość obługi kamery.

## 2. Działanie

### Struktura MapRenderer
Struktura pomocnicza, odostępniająca metodę `draw(...)` rysująca teksturę mapy na ekranie.

### Struktura GameScreen
Struktura ogólna, posiadająca struktury ***MapView*** oraz ***MapCamera***. Odpowiada za obsługę całej funkcjonalności w najwyższej możliwej abstrakcji poprzez metodę `run(...)`.
W ogólności mapa jest rysowana w nieskończonej abstrakcyjnej przestrzeni (klasyczny układ współrzędnych z zamienionymi zwrotami osi pionowej), a obiekt ***Camera2D*** z biblioteki *macroquad* odpowiada za wycięcie odpowiedniej części tej przestrzeni i narysowanie jej wprost na monitorze (metody `set_camera(...) oraz camera_default(...)`).
