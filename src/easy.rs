//! API wrappers for common use cases like highlighting strings and
//! files without caring about intermediate semantic representation
//! and caching.

use crate::parsing::{ScopeStack, ParseState, SyntaxReference, SyntaxSet, ScopeStackOp};
use crate::highlighting::{Highlighter, HighlightState, HighlightIterator, Theme, Style};
use std::io::{self, BufReader};
use std::fs::File;
use std::path::Path;
// use util::debug_print_ops;

/// Simple way to go directly from lines of text to colored tokens.
///
/// Depending on how you load the syntaxes (see the [`SyntaxSet`] docs), this can either take
/// strings with trailing `\n`s or without.
///
/// [`SyntaxSet`]: ../parsing/struct.SyntaxSet.html
///
/// # Examples
///
/// Prints colored lines of a string to the terminal
///
/// ```
/// use syntect::easy::HighlightLines;
/// use syntect::parsing::SyntaxSet;
/// use syntect::highlighting::{ThemeSet, Style};
/// use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
///
/// // Load these once at the start of your program
/// let ps = SyntaxSet::load_defaults_newlines();
/// let ts = ThemeSet::load_defaults();
///
/// let syntax = ps.find_syntax_by_extension("rs").unwrap();
/// let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
/// let s = "pub struct Wow { hi: u64 }\nfn blah() -> u64 {}";
/// for line in LinesWithEndings::from(s) { // LinesWithEndings enables use of newlines mode
///     let ranges: Vec<(Style, &str)> = h.highlight(line, &ps);
///     let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
///     print!("{}", escaped);
/// }
/// ```
pub struct HighlightLines<'a> {
    highlighter: Highlighter<'a>,
    parse_state: ParseState,
    highlight_state: HighlightState,
}

impl<'a> HighlightLines<'a> {
    pub fn new(syntax: &SyntaxReference, theme: &'a Theme) -> HighlightLines<'a> {
        let highlighter = Highlighter::new(theme);
        let highlight_state = HighlightState::new(&highlighter, ScopeStack::new());
        HighlightLines {
            highlighter,
            parse_state: ParseState::new(syntax),
            highlight_state,
        }
    }

    /// Highlights a line of a file
    pub fn highlight<'b>(&mut self, line: &'b str, syntax_set: &SyntaxSet) -> Vec<(Style, &'b str)> {
        // println!("{}", self.highlight_state.path);
        let ops = self.parse_state.parse_line(line, syntax_set);
        // use util::debug_print_ops;
        // debug_print_ops(line, &ops);
        let iter =
            HighlightIterator::new(&mut self.highlight_state, &ops[..], line, &self.highlighter);
        iter.collect()
    }
}

/// Convenience struct containing everything you need to highlight a file
///
/// Use the `reader` to get the lines of the file and the `highlight_lines` to highlight them. See
/// the [`new`] method docs for more information.
///
/// [`new`]: #method.new
pub struct HighlightFile<'a> {
    pub reader: BufReader<File>,
    pub highlight_lines: HighlightLines<'a>,
}

impl<'a> HighlightFile<'a> {
    /// Constructs a file reader and a line highlighter to get you reading files as fast as possible.
    ///
    /// This auto-detects the syntax from the extension and constructs a [`HighlightLines`] with the
    /// correct syntax and theme.
    ///
    /// [`HighlightLines`]: struct.HighlightLines.html
    ///
    /// # Examples
    ///
    /// Using the `newlines` mode is a bit involved but yields more robust and glitch-free highlighting,
    /// as well as being slightly faster since it can re-use a line buffer.
    ///
    /// ```
    /// use syntect::parsing::SyntaxSet;
    /// use syntect::highlighting::{ThemeSet, Style};
    /// use syntect::util::as_24_bit_terminal_escaped;
    /// use syntect::easy::HighlightFile;
    /// use std::io::BufRead;
    ///
    /// # use std::io;
    /// # fn foo() -> io::Result<()> {
    /// let ss = SyntaxSet::load_defaults_newlines();
    /// let ts = ThemeSet::load_defaults();
    ///
    /// let mut highlighter = HighlightFile::new("testdata/highlight_test.erb", &ss, &ts.themes["base16-ocean.dark"]).unwrap();
    /// let mut line = String::new();
    /// while highlighter.reader.read_line(&mut line)? > 0 {
    ///     {
    ///         let regions: Vec<(Style, &str)> = highlighter.highlight_lines.highlight(&line, &ss);
    ///         print!("{}", as_24_bit_terminal_escaped(&regions[..], true));
    ///     } // until NLL this scope is needed so we can clear the buffer after
    ///     line.clear(); // read_line appends so we need to clear between lines
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// This example uses `reader.lines()` to get lines without a newline character, it's simpler but may break on rare tricky cases.
    ///
    /// ```
    /// use syntect::parsing::SyntaxSet;
    /// use syntect::highlighting::{ThemeSet, Style};
    /// use syntect::util::as_24_bit_terminal_escaped;
    /// use syntect::easy::HighlightFile;
    /// use std::io::BufRead;
    ///
    /// let ss = SyntaxSet::load_defaults_nonewlines();
    /// let ts = ThemeSet::load_defaults();
    ///
    /// let mut highlighter = HighlightFile::new("testdata/highlight_test.erb", &ss, &ts.themes["base16-ocean.dark"]).unwrap();
    /// for maybe_line in highlighter.reader.lines() {
    ///     let line = maybe_line.unwrap();
    ///     let regions: Vec<(Style, &str)> = highlighter.highlight_lines.highlight(&line, &ss);
    ///     println!("{}", as_24_bit_terminal_escaped(&regions[..], true));
    /// }
    /// ```
    pub fn new<P: AsRef<Path>>(path_obj: P,
                               ss: &SyntaxSet,
                               theme: &'a Theme)
                               -> io::Result<HighlightFile<'a>> {
        let path: &Path = path_obj.as_ref();
        let f = File::open(path)?;
        let syntax = ss.find_syntax_for_file(path)?
            .unwrap_or_else(|| ss.find_syntax_plain_text());

        Ok(HighlightFile {
            reader: BufReader::new(f),
            highlight_lines: HighlightLines::new(syntax, theme),
        })
    }
}

/// Iterator over the ranges of a line which a given the operation from the parser applies.
///
/// Use [`ScopeRegionIterator`] to obtain directly regions (`&str`s) from the line.
///
/// To use, just keep your own [`ScopeStack`] and then `ScopeStack.apply(op)` the operation that is
/// yielded at the top of your `for` loop over this iterator. Now you have a substring of the line
/// and the scope stack for that token.
///
/// See the `synstats.rs` example for an example of using this iterator.
///
/// **Note:** This will often return empty ranges, just `continue` after applying the op if you
/// don't want them.
///
/// [`ScopeStack`]: ../parsing/struct.ScopeStack.html
/// [`ScopeRegionIterator`]: ./struct.ScopeRegionIterator.html
#[derive(Debug)]
pub struct ScopeRangeIterator<'a> {
    ops: &'a [(usize, ScopeStackOp)],
    line: &'a str,
    index: usize,
    last_str_index: usize,
}

impl<'a> ScopeRangeIterator<'a> {
    pub fn new(ops: &'a [(usize, ScopeStackOp)], line: &'a str) -> ScopeRangeIterator<'a> {
        ScopeRangeIterator {
            ops,
            line,
            index: 0,
            last_str_index: 0,
        }
    }
}

static NOOP_OP: ScopeStackOp = ScopeStackOp::Noop;

impl<'a> Iterator for ScopeRangeIterator<'a> {
    type Item = (std::ops::Range<usize>, &'a ScopeStackOp);
    fn next(&mut self) -> Option<Self::Item> {
        if self.index > self.ops.len() {
            return None;
        }

        // region extends up to next operation (ops[index]) or string end if there is none
        // note the next operation may be at, last_str_index, in which case the region is empty
        let next_str_i = if self.index == self.ops.len() {
            self.line.len()
        } else {
            self.ops[self.index].0
        };
        let range = self.last_str_index..next_str_i;
        self.last_str_index = next_str_i;

        // the first region covers everything before the first op, which may be empty
        let op = if self.index == 0 {
            &NOOP_OP
        } else {
            &self.ops[self.index - 1].1
        };

        self.index += 1;
        Some((range, op))
    }
}

/// A convenience wrapper over [`ScopeRangeIterator`] to return `&str`s directly.
///
/// To use, just keep your own [`ScopeStack`] and then `ScopeStack.apply(op)` the operation that is
/// yielded at the top of your `for` loop over this iterator. Now you have a substring of the line
/// and the scope stack for that token.
///
/// See the `synstats.rs` example for an example of using this iterator.
///
/// **Note:** This will often return empty regions, just `continue` after applying the op if you
/// don't want them.
///
/// [`ScopeStack`]: ../parsing/struct.ScopeStack.html
/// [`ScopeRangeIterator`]: ./struct.ScopeRangeIterator.html
#[derive(Debug)]
pub struct ScopeRegionIterator<'a> {
    range_iter: ScopeRangeIterator<'a>,
}

impl<'a> ScopeRegionIterator<'a> {
    pub fn new(ops: &'a [(usize, ScopeStackOp)], line: &'a str) -> ScopeRegionIterator<'a> {
        ScopeRegionIterator {
            range_iter: ScopeRangeIterator::new(ops, line),
        }
    }
}

impl<'a> Iterator for ScopeRegionIterator<'a> {
    type Item = (&'a str, &'a ScopeStackOp);
    fn next(&mut self) -> Option<Self::Item> {
        let (range, op) = self.range_iter.next()?;
        Some((&self.range_iter.line[range], op))
    }
}

#[cfg(all(feature = "assets", any(feature = "dump-load", feature = "dump-load-rs")))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::{SyntaxSet, ParseState, ScopeStack};
    use crate::highlighting::ThemeSet;
    use std::str::FromStr;

    #[test]
    fn can_highlight_lines() {
        let ss = SyntaxSet::load_defaults_nonewlines();
        let ts = ThemeSet::load_defaults();
        let syntax = ss.find_syntax_by_extension("rs").unwrap();
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
        let ranges = h.highlight("pub struct Wow { hi: u64 }", &ss);
        assert!(ranges.len() > 4);
    }

    #[test]
    fn can_highlight_file() {
        let ss = SyntaxSet::load_defaults_nonewlines();
        let ts = ThemeSet::load_defaults();
        HighlightFile::new("testdata/highlight_test.erb",
                           &ss,
                           &ts.themes["base16-ocean.dark"])
            .unwrap();
    }

    #[test]
    fn can_find_regions() {
        let ss = SyntaxSet::load_defaults_nonewlines();
        let mut state = ParseState::new(ss.find_syntax_by_extension("rb").unwrap());
        let line = "lol =5+2";
        let ops = state.parse_line(line, &ss);

        let mut stack = ScopeStack::new();
        let mut token_count = 0;
        for (s, op) in ScopeRegionIterator::new(&ops, line) {
            stack.apply(op);
            if s.is_empty() { // in this case we don't care about blank tokens
                continue;
            }
            if token_count == 1 {
                assert_eq!(stack, ScopeStack::from_str("source.ruby keyword.operator.assignment.ruby").unwrap());
                assert_eq!(s, "=");
            }
            token_count += 1;
            println!("{:?} {}", s, stack);
        }
        assert_eq!(token_count, 5);
    }

    #[test]
    fn can_find_regions_with_trailing_newline() {
        let ss = SyntaxSet::load_defaults_newlines();
        let mut state = ParseState::new(ss.find_syntax_by_extension("rb").unwrap());
        let lines = ["# hello world\n", "lol=5+2\n"];
        let mut stack = ScopeStack::new();

        for line in lines.iter() {
            let ops = state.parse_line(line, &ss);
            println!("{:?}", ops);

            let mut iterated_ops: Vec<&ScopeStackOp> = Vec::new();
            for (_, op) in ScopeRegionIterator::new(&ops, line) {
                stack.apply(op);
                iterated_ops.push(op);
                println!("{:?}", op);
            }

            let all_ops = ops.iter().map(|t| &t.1);
            assert_eq!(all_ops.count(), iterated_ops.len() - 1); // -1 because we want to ignore the NOOP
        }
    }
}
