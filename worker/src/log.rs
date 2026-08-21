use crate::proc;
use rebuilderd_common::errors::*;
use std::collections::VecDeque;

const TAIL_SIZE_LIMIT: usize = 1024 * 1024 * 2; // 2 MiB

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Accepted,
    Truncated(usize),
}

pub struct Buffer {
    front: Vec<u8>,
    tail: VecDeque<u8>,
    extra_status_msg: Option<String>,
    front_size_limit: Option<usize>,
    tail_size_limit: usize,
    truncated: bool,
}

impl Buffer {
    pub fn new(front_size_limit: Option<usize>, tail_size_limit: Option<usize>) -> Self {
        Self {
            front: Vec::new(),
            tail: VecDeque::new(),
            extra_status_msg: None,
            front_size_limit,
            tail_size_limit: tail_size_limit.unwrap_or(TAIL_SIZE_LIMIT),
            truncated: false,
        }
    }

    pub fn from_opts(opts: &proc::Options) -> Self {
        Self::new(opts.front_size_limit, opts.tail_size_limit)
    }

    /// Append data to the log buffer until the size limit is reached (if any).
    /// Further data is pushed to a tail ring buffer that keeps the last N bytes
    /// of the log output. Returns a truncation indicator if the size limit was
    /// reached for the first time.
    pub fn push_bytes(&mut self, slice: &[u8]) -> Status {
        if !self.truncated {
            if let Some(front_size_limit) = self.front_size_limit
                && let remaining_front_space = front_size_limit.saturating_sub(self.front.len())
                && let Some((front_slice, tail_slice)) =
                    slice.split_at_checked(remaining_front_space)
                && !tail_slice.is_empty()
            {
                // Slice exceeds the front size limit, add the data that still
                // fits in the front, and push the rest to the tail, indicate
                // that truncation happened

                warn!(
                    "Exceeding output limit: output={}, slice={}, limit={}",
                    self.front.len(),
                    slice.len(),
                    front_size_limit
                );

                self.front.extend(front_slice);
                self.truncated = true;
                self.push_tail(tail_slice);

                Status::Truncated(front_size_limit)
            } else {
                // Entire slice fits in the front
                self.front.extend(slice);
                Status::Accepted
            }
        } else {
            // The front is already full, push into the tail ring buffer
            self.push_tail(slice);
            Status::Accepted
        }
    }

    /// Append data to the tail ring buffer, and remove the oldest data if the
    /// size limit is exceeded
    fn push_tail(&mut self, slice: &[u8]) {
        self.tail.extend(slice);

        let excess = self.tail.len().saturating_sub(self.tail_size_limit);
        if excess > 0 {
            self.tail.drain(..excess);
        }
    }

    /// Add a truncation message to the front buffer
    ///
    /// The size limit does not apply to this message
    pub fn truncate(&mut self, reason: &str) {
        self.front.extend(b"\n\n");
        self.front.extend(reason.as_bytes());
        self.front.extend(b"\n\n");
        self.truncated = true;
    }

    /// Append an extra status message to the very end of the log output
    pub fn extra_status_msg(&mut self, msg: String) {
        self.extra_status_msg = Some(msg);
    }

    /// Check if the log buffer is entirely empty (both front and tail)
    pub fn is_empty(&self) -> bool {
        self.front.is_empty() && self.tail.is_empty() && self.extra_status_msg.is_none()
    }

    /// Get the total length of the log buffer (front + tail + extra status message)
    pub fn len(&self) -> usize {
        let mut len = self.front.len().saturating_add(self.tail.len());

        if let Some(msg) = &self.extra_status_msg {
            if len > 0 {
                // the two newlines before the extra message
                len = len.saturating_add(2);
            }
            len = len.saturating_add(msg.len());
        }

        len
    }

    /// Convert the log buffers into a single UTF-8 string, including the extra status message if present
    pub fn make_string(&mut self) -> String {
        let mut output = String::from_utf8_lossy(&self.front).into_owned();

        if !self.tail.is_empty() {
            let tail = self.tail.make_contiguous();
            let tail = String::from_utf8_lossy(tail);
            output.push_str(&tail);
        }

        if let Some(extra_msg) = &self.extra_status_msg {
            if !output.is_empty() {
                output.push_str("\n\n");
            }
            output.push_str(extra_msg);
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tail_excess() {
        let mut buffer = Buffer::new(None, Some(5));

        // Push some data
        buffer.push_tail(b"123");
        assert_eq!(buffer.tail.as_slices(), ("123".as_bytes(), "".as_bytes()));

        // Exceed size limit
        buffer.push_tail(b"456");
        assert_eq!(buffer.tail.as_slices(), ("23456".as_bytes(), "".as_bytes()));

        // Entire message exceeds size limit
        buffer.push_tail(b"789ABCD");
        assert_eq!(buffer.tail.as_slices(), ("9ABCD".as_bytes(), "".as_bytes()));

        // Exceed size limit one-by-one
        buffer.push_tail(b"1");
        assert_eq!(buffer.tail.as_slices(), ("ABCD1".as_bytes(), "".as_bytes()));
        buffer.push_tail(b"2");
        assert_eq!(buffer.tail.as_slices(), ("BCD12".as_bytes(), "".as_bytes()));
        buffer.push_tail(b"3");
        assert_eq!(buffer.tail.as_slices(), ("CD123".as_bytes(), "".as_bytes()));
        buffer.push_tail(b"4");
        assert_eq!(buffer.tail.as_slices(), ("D123".as_bytes(), "4".as_bytes()));
        buffer.push_tail(b"5");
        assert_eq!(buffer.tail.as_slices(), ("123".as_bytes(), "45".as_bytes()));
        buffer.push_tail(b"6");
        assert_eq!(buffer.tail.as_slices(), ("23".as_bytes(), "456".as_bytes()));
        buffer.push_tail(b"7");
        assert_eq!(buffer.tail.as_slices(), ("3".as_bytes(), "4567".as_bytes()));
        buffer.push_tail(b"8");
        assert_eq!(buffer.tail.as_slices(), ("45678".as_bytes(), "".as_bytes()));
    }

    // The musicians of the town of Bremen
    //
    // Copyright 1819 The Brothers Grimm
    // Copyright 1855 Matilda Louisa Davis
    const DATA: &str = "There was once a man who had an ass which had served him faithfully many
years, but his strength being now exhausted, he became daily less and less
useful to his master, who accordingly began to grudge him his food. The ass
observing that evil was brewing for him, ran away, and took the road to
Bremen. “There,” said he, “I can obtain a place as town musician.” After
proceeding for some time, he found a greyhound lying by the roadside,
gasping as if he had run himself out of breath. “What is the matter?”
inquired the ass ; “why do you gasp so?” “Ah!” said the hound, “I am old,
and grow weaker every day, and can no longer hunt so well as I did; my
master therefore wished to kill me, but I have left him in the lurch,
although I cannot tell in the least how I shall earn my bread for the
future.” “I will tell you,” replied the ass. “I am going to Bremen to become
a musician there ; go with me and take up music; I will play on the lute,
and you can beat the kettledrum.” The hound was much obliged for the
suggestion, and they proceeded together ; before long they saw a cat sitting
by the wayside, with a very doleful countenance. “Now what is the grievance,
old lick-paw?” asked the ass. “Who could be merry, I should be glad to know,
when their neck was in danger?” replied the cat; “I am now old, and my teeth
fail, therefore I would rather sit by the fire than run after the mice, and
I heard my mistress give orders for me to be hung ; so I am cast upon the
world, and cannot see my way how I am to live.” ”Go with us to Bremen ; you
understand serenades very well, so you may become one of the town
musicians.” The cat was only too happy to accept the offer, and joined the
other two.";

    #[test]
    fn test_capture_no_limit() {
        let mut buffer = Buffer::new(None, None);
        buffer.push_bytes(DATA.as_bytes());
        assert_eq!(buffer.make_string(), DATA);
    }

    #[test]
    fn test_capture_100_byte_front_limit() {
        let mut buffer = Buffer::new(Some(100), None);
        buffer.push_bytes(DATA.as_bytes());
        assert_eq!(buffer.make_string(), DATA);
    }

    #[test]
    fn test_capture_char_boundary() {
        // Use an index that is inside a char boundary
        let idx = 305;
        assert!(str::from_utf8(&DATA.as_bytes()[..idx]).is_err());

        // Capture
        let mut buffer = Buffer::new(Some(idx), None);
        buffer.push_bytes(&DATA.as_bytes()[..350]);

        // There's invalid utf8 markers on both sides of the slice.
        // Although in this specific test we could try to stitch them back
        // together, there may be missing data between front and back, or
        // additional data in the middle
        let buffer = buffer.make_string();
        assert_eq!(
            buffer,
            "There was once a man who had an ass which had served him faithfully many
years, but his strength being now exhausted, he became daily less and less
useful to his master, who accordingly began to grudge him his food. The ass
observing that evil was brewing for him, ran away, and took the road to
Bremen. ���There,” said he, “I can obtain a place "
        );
        assert_eq!(buffer.len(), 356);
    }

    #[test]
    fn test_capture_with_tail_oneshot() {
        let mut buffer = Buffer::new(Some(30), Some(50));
        buffer.push_bytes(DATA.as_bytes());
        let buffer = buffer.make_string();
        assert_eq!(
            buffer,
            "There was once a man who had appy to accept the offer, and joined the\nother two."
        );
        assert_eq!(buffer.len(), 80);
    }

    #[test]
    fn test_capture_with_tail_append() {
        let mut buffer = Buffer::new(Some(30), Some(50));
        buffer.push_bytes(DATA.as_bytes());
        buffer.push_bytes(b"Hello World");
        let buffer = buffer.make_string();
        assert_eq!(
            buffer,
            "There was once a man who had apt the offer, and joined the\nother two.Hello World"
        );
        assert_eq!(buffer.len(), 80);
    }

    #[test]
    fn test_capture_in_chunks() {
        let mut buffer = Buffer::new(Some(30), Some(30));
        for window in DATA.as_bytes().chunks(20) {
            buffer.push_bytes(window);
        }
        let buffer = buffer.make_string();
        assert_eq!(
            buffer,
            "There was once a man who had afer, and joined the\nother two."
        );
        assert_eq!(buffer.len(), 60);
    }

    #[test]
    fn test_capture_empty() {
        let mut buffer = Buffer::new(Some(10), Some(10));
        let buffer = buffer.make_string();
        assert_eq!(buffer, "");
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_capture_empty_extra_msg() {
        let mut buffer = Buffer::new(Some(10), Some(10));
        buffer.extra_status_msg("Extra message\n".to_string());
        assert_eq!(buffer.len(), 14);
        let buffer = buffer.make_string();
        assert_eq!(buffer, "Extra message\n");
        assert_eq!(buffer.len(), 14);
    }

    #[test]
    fn test_capture_append_extra_msg() {
        let mut buffer = Buffer::new(Some(10), Some(10));
        buffer.extra_status_msg("Extra message\n".to_string());
        buffer.push_bytes(b"Hello World");
        assert_eq!(buffer.len(), 27);
        let buffer = buffer.make_string();
        assert_eq!(buffer, "Hello World\n\nExtra message\n");
        assert_eq!(buffer.len(), 27);
    }
}
