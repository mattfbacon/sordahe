# Sordahe

Stenography for Wayland.

## Usage

Run the program with `--help`.

But basically, the program can currently run in two different modes:

### As an input method

This mode captures the normal keyboard and translates it into a stenotype keyboard using the input method API.

This API is not as well-supported as the virtual keyboard (because it requires client support) so some applications will struggle with it, specifically the backspacing part. However, it allows you to use a normal keyboard for stenotype.

### As a virtual keyboard

This mode allows you to use a dedicated stenotype keyboard. It will try to discover a Nolltronics device (the steno keyboard I happen to have on hand) but you can specify a device with `-d/--device`. Currently only the Gemini protocol is implemented.

The output of the stenotype engine will be synthesized into key presses on the virtual keyboard, allowing for almost any app to support it.

In either mode, the program loads the dictionary from `dict.json` in the current directory, or from the path specified with `-D/--dict`.

## Name

`sorda'e` means "many presses" in Lojban.

## Credits

The dictionary comes from Plover, extracted from the web-app.
The word list comes from <https://github.com/dolph/dictionary>, specifically `enable1.txt`.

## License

AGPL-3.0-or-later
