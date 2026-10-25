#include <TM1638plus_Model2.h>
#include "USB.h"
#include "USBMIDI.h"

#if ARDUINO_USB_MODE
#error Set usb mode to USB-OTG (TinyUSB) so the ESP can act as a USB MIDI device
#endif

#define STROBE_PIN 4
#define CLOCK_PIN  5
#define DIO_PIN    6

#define JOY_X_PIN    1
#define JOY_Y_PIN    2
#define JOY_SW_PIN   7
#define JOY_DEADZONE 100

#define BASE_NOTE  60
#define BASE_VELOCITY 100
#define VELOCITY_RATE 1.0f

TM1638plus_Model2 tm(STROBE_PIN, CLOCK_PIN, DIO_PIN, false, false);

USBMIDI MIDI("ESP MIDI Synth");

uint16_t lastKeys = 0;
int joyCenterX, joyCenterY;
int16_t lastBend = 0;
float velocity = BASE_VELOCITY;
uint8_t shownVelocity = BASE_VELOCITY;

int calibrateAxis(uint8_t pin) {
  long sum = 0;
  for (uint8_t i = 0; i < 16; i++) sum += analogRead(pin);
  return sum / 16;
}

float readAxis(uint8_t pin, int center) {
  int offset = analogRead(pin) - center;
  if (abs(offset) < JOY_DEADZONE) return 0;
  float range = offset > 0 ? 4095 - center : center;
  return constrain(offset / range, -1.0f, 1.0f);
}

void setup() {
  tm.displayBegin();
  tm.DisplayStr("HELLO");

  joyCenterX = calibrateAxis(JOY_X_PIN);
  joyCenterY = calibrateAxis(JOY_Y_PIN);
  pinMode(JOY_SW_PIN, INPUT_PULLUP);

  MIDI.begin();
  USB.begin();
}

void loop() {
  int16_t bend = readAxis(JOY_X_PIN, joyCenterX) * 8191;
  if (bend != lastBend && (bend == 0 || abs(bend - lastBend) >= 64)) {
    MIDI.pitchBend(bend);
    lastBend = bend;
  }

   velocity = constrain(velocity + readAxis(JOY_Y_PIN, joyCenterY) * VELOCITY_RATE, 1.0f, 127.0f);
  if (digitalRead(JOY_SW_PIN) == LOW) velocity = BASE_VELOCITY;

  uint16_t keys = tm.ReadKey16Two();

  if ((uint8_t)velocity != shownVelocity && keys == 0) {
    char text[9];
    snprintf(text, sizeof(text), "VEL %3u", (uint8_t)velocity);
    tm.DisplayStr(text);
    shownVelocity = velocity;
  }
  uint16_t changed = keys ^ lastKeys;
  if (changed) {
    for (uint8_t i = 0; i < 16; i++) {
      uint16_t mask = 1 << i;
      if (!(changed & mask)) continue;
      if (keys & mask) {
        MIDI.noteOn(BASE_NOTE + i, (uint8_t)velocity);
      } else {
        MIDI.noteOff(BASE_NOTE + i);
      }
    }

    if (keys == 0) {
      tm.DisplayStr("HELLO");
    } else {
      uint8_t lowest = 0;
      while (!(keys & (1 << lowest))) lowest++;
      char text[9];
      snprintf(text, sizeof(text), "KEY %2u", lowest + 1);
      tm.DisplayStr(text);
    }
    lastKeys = keys;
  }
  delay(20);
}
