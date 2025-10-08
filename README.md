# MIDIval Renaissance

This repository contains firmware for a device which allows the [Micromoog Model 2090](https://en.wikipedia.org/wiki/Micromoog), a monophonic analog synthesizer produced by Moog Music from 1975 to 1979, to respond to <abbr title="Musical/ Instrument Digital Interface">MIDI</abbr> input. It works by translating MIDI messages into electrical signals compatible with the Moog Open System, which is essentially a flavor of [CV/gate](https://en.wikipedia.org/wiki/CV/gate). In addition to enabling the Micromoog to be controlled externally, it seeks to extend the capabilities of the synthesizer without modifying it per se.

Presently the device is based on the [Nucleo-F767ZI development board](https://www.st.com/en/evaluation-tools/nucleo-f767zi.html), which is powered by an F7-series STM32 microcontroller.

While I'm open to the MIDIval Renaissance someday serving as an adapter for other synthesizers, my focus is limited to the Micromoog until I feel I've accomplished everything I want to for this instrument.

## Features

The organization of this section mirrors the inputs of the Moog Open System, in the order in which they appear in the [Micromoog Operation Manual (manual 14-003)](https://archive.org/details/JL11295).

### Audio Input

The MIDIval Renaissance does not utilize the Micrmoog's audio input, and there are no plans for it to do so.

### S-Trig Input

The purpose of the switch trigger is simply to trigger the synth's loudness and filter contours, i.e., to tell it when to sound and when to rest. The MIDIval Renaissance currently implements this exactly as does the Micromoog. That is, the contours are reset anytime there is a break between notes, but notes played legato will be played within the same envelope contours.

#### Extensions

- [ ] On the roadmap, but not yet implemented, is a setting which would reset the contours each time a different note is voiced, regardless of how it is articulated. This means you'd get your filter sweep or volume fade-in on every new note, even if you didn't play precisely enough to leave a tiny gap between releasing one key and pressing the next.

### Keyboard Input

The KBD OUTPUT jack of the Micromoog is a dual-purpose jack which also receives input. When in input mode, it receives a signal indicating the note to voice. Since the Micromoog is a monophonic instrument with low-note priority, if multiple notes are sent by the controller, only the lowest note will be voiced by default.

Glide is not yet implemented.

#### Extensions

- [x] A setting allows the performer to set the note priority:
  - First: Prioritizes notes based on the order in which they are received. Notes played earlier will be voiced over later ones.
  - Last: Prioritizes notes based on the order in which they are received. Notes played later will be voiced over earlier ones.
  - Low: Prioritizes notes based on pitch. Lower notes (e.g., those on the left side of the keyboard) will be voiced over higher ones. This is the default.
  - High: Prioritizes notes based on pitch. Higher notes (e.g., those on the right side of the keyboard) will be voiced over lower ones.
- [ ] The "chord cleanup" setting inserts a slight delay between MIDI input and eletrical output to account for human imprecision. If you are playing chords on your controller and have your note priority set to low, it stands to reason that you're expecting "bass lines for free" from the MIDIval Renaissance/Micromoog combo. This setting enables "close enough" timing for all the keypresses associated with the performance of a chord so that the Micromoog doesn't play the third or the fifth for a split second on those occasions where they land before the root note.
- [ ] Arpeggiation of chords.

### Filter Input

This input will be considered after the S-Trig and Keyboard inputs are fully implemented.

### Oscillator Input

This input will be considered after the S-Trig and Keyboard inputs are fully implemented.

### Modulation

This input will be considered after the S-Trig and Keyboard inputs are fully implemented.

## Known Issues/Limitations

- Presently the device does not operate in USB Host mode. Thus, it can only be used as a peripheral. If you're looking to feed MIDI from a laptop-hosted <abbr title="digital audio workstation">DAW</abbr> to a Micromoog via the MIDIval Renaissance, this presents no problem for you. If instead you wish to live-play MIDI from a controller, you'll need to use a laptop as an intermediary. For example, the following screenshot shows I am using [Ardour](https://ardour.org/) as my DAW, and that I've routed MIDI output from the (Sequential Prophet) Rev2 to the input for the MIDIval Renaissance: ![Ardour configuration screenshot](./ardour-midi-connection-manager.png)

## Usage

## License

MIDIval Renaissance is distributed under the terms of both the [MIT license](./LICENSE-MIT)
and the [Apache License (Version 2.0)](./LICENSE-APACHE).

Any contribution intentionally submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Contributing

Issues, pull requests, feature requests, and constructive criticism are welcome.

I will also accept your donated or lent synthesizer ;-)
