pub mod special {
    /// Start a text section
    pub const TEXT_START: u8 = 0x00;

    /// A space character
    pub const SPACE: u8 = 0x7F;

    /// Move down a line
    pub const LINE_DOWN: u8 = 0x4E;

    /// Start writing to the bottom line
    pub const BOTTOM_LINE: u8 = 0x4F;

    /// Start a new paragraph
    pub const PARAGRAPH: u8 = 0x51;

    /// Scroll to the next line
    pub const SCROLL_LINE: u8 = 0x55;

    /// End the message box
    pub const END_MSG: u8 = 0x57;

    /// Prompt player to end text box
    pub const END_PROMPT: u8 = 0x58;

    /// Terminates the string
    pub const TERMINATOR: u8 = 0x50;
}

pub struct Encoder<'a> {
    base: &'a str,
}

impl<'a> Encoder<'a> {
    pub fn new(text: &'a str) -> Encoder<'a> {
        Encoder { base: text }
    }
}

impl<'a> Iterator for Encoder<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        fn encode_char(char_: char) -> u8 {
            let val = char_ as u8;
            match char_ {
                'A'...'Z' => 0x80 + (val - 'A' as u8),

                '('       => 0x8A,
                ')'       => 0x8B,
                ':'       => 0x8C,
                ';'       => 0x8D,
                '['       => 0x8E,
                ']'       => 0x8F,

                'a'...'z' => 0xA0 + (val - 'a' as u8),

                '\''      => 0xE0, // TODO: We might want to handle this case more carefully
                '-'       => 0xE3,
                '?'       => 0xE6,
                '!'       => 0xE7,
                '.'       => 0xE8,
                '/'       => 0xF3,
                ','       => 0xF4,

                '0'...'9' => 0xF6 + (val - '0' as u8),

                // Special characters
                ' '       => special::SPACE,
                '\n'      => special::LINE_DOWN, // FIXME: Handle this better
                _         => 0xE6, // Use ? for invalid characters
            }
        }

        if let Some((char_, rest)) = self.base.slice_shift_char() {
            self.base = rest;
            return Some(encode_char(char_));
        }

        None
    }
}