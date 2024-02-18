use std::result;

use csv_core::WriterBuilder as CoreWriterBuilder;
use csv_core::{self, WriteResult, Writer as CoreWriter};
cfg_if::cfg_if! {
if #[cfg(feature = "tokio")] {
    use tokio::io::{self, AsyncWrite, AsyncWriteExt};
} else {
    use futures::io::{self, AsyncWrite, AsyncWriteExt};
}}
    

use crate::{QuoteStyle, Terminator};
use crate::byte_record::ByteRecord;
use crate::error::{Error, ErrorKind, IntoInnerError, Result};

#[cfg(feature = "with_serde")]
pub mod mwtr_serde;

cfg_if::cfg_if! {
if #[cfg(feature = "tokio")] {
    pub mod awtr_tokio;
} else {
    pub mod awtr_futures;
}}
        
#[cfg(all(feature = "with_serde", not(feature = "tokio")))]
pub mod aser_futures;
    
#[cfg(all(feature = "with_serde", feature = "tokio"))]
pub mod aser_tokio;

//-//////////////////////////////////////////////////////////////////////////////////////////////
//-// Builder
//-//////////////////////////////////////////////////////////////////////////////////////////////

/// Builds a CSV writer with various configuration knobs.
///
/// This builder can be used to tweak the field delimiter, record terminator
/// and more. Once a CSV `AsyncWriter` is built, its configuration cannot be
/// changed.
#[derive(Debug)]
pub struct AsyncWriterBuilder {
    builder: CoreWriterBuilder,
    capacity: usize,
    flexible: bool,
    has_headers: bool,
}

impl Default for AsyncWriterBuilder {
    fn default() -> AsyncWriterBuilder {
        AsyncWriterBuilder {
            builder: CoreWriterBuilder::default(),
            capacity: 8 * (1 << 10),
            flexible: false,
            has_headers: true,
        }
    }
}

impl AsyncWriterBuilder {
    /// Create a new builder for configuring CSV writing.
    ///
    /// To convert a builder into a writer, call one of the methods starting
    /// with `from_`.
    ///
    /// # Example
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::AsyncWriterBuilder;
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new().create_writer(vec![]);
    ///     wtr.write_record(&["a", "b", "c"]).await?;
    ///     wtr.write_record(&["x", "y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "a,b,c\nx,y,z\n");
    ///     Ok(())
    /// }
    /// ```
    pub fn new() -> AsyncWriterBuilder {
        AsyncWriterBuilder::default()
    }

    /// The field delimiter to use when writing CSV.
    ///
    /// The default is `b','`.
    ///
    /// # Example
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::AsyncWriterBuilder;
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .delimiter(b';')
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "b", "c"]).await?;
    ///     wtr.write_record(&["x", "y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "a;b;c\nx;y;z\n");
    ///     Ok(())
    /// }
    /// ```
    pub fn delimiter(&mut self, delimiter: u8) -> &mut AsyncWriterBuilder {
        self.builder.delimiter(delimiter);
        self
    }
    /// Whether to write a header row before writing any other row.
    ///
    /// When this is enabled and the `serialize` method is used to write data
    /// with something that contains field names (i.e., a struct), then a
    /// header row is written containing the field names before any other row
    /// is written.
    ///
    /// This option has no effect when using other methods to write rows. That
    /// is, if you don't use `serialize`, then you must write your header row
    /// explicitly if you want a header row.
    ///
    /// This is enabled by default.
    ///
    // / # Example: with headers
    // /
    // / This shows how the header will be automatically written from the field
    // / names of a struct.
    // /
    // / ```
    // / use std::error::Error;
    // /
    // / use csv::WriterBuilder;
    // / use serde::Serialize;
    // /
    // / #[derive(Serialize)]
    // / struct Row<'a> {
    // /     city: &'a str,
    // /     country: &'a str,
    // /     // Serde allows us to name our headers exactly,
    // /     // even if they don't match our struct field names.
    // /     #[serde(rename = "popcount")]
    // /     population: u64,
    // / }
    // /
    // / # fn main() { example().unwrap(); }
    // / fn example() -> Result<(), Box<dyn Error>> {
    // /     let mut wtr = WriterBuilder::new().from_writer(vec![]);
    // /     wtr.serialize(Row {
    // /         city: "Boston",
    // /         country: "United States",
    // /         population: 4628910,
    // /     })?;
    // /     wtr.serialize(Row {
    // /         city: "Concord",
    // /         country: "United States",
    // /         population: 42695,
    // /     })?;
    // /
    // /     let data = String::from_utf8(wtr.into_inner()?)?;
    // /     assert_eq!(data, "\
    // / city,country,popcount
    // / Boston,United States,4628910
    // / Concord,United States,42695
    // / ");
    // /     Ok(())
    // / }
    // / ```
    // /
    // / # Example: without headers
    // /
    // / This shows that serializing things that aren't structs (in this case,
    // / a tuple struct) won't result in a header row being written. This means
    // / you usually don't need to set `has_headers(false)` unless you
    // / explicitly want to both write custom headers and serialize structs.
    // /
    // / ```
    // / use std::error::Error;
    // / use csv::WriterBuilder;
    // /
    // / # fn main() { example().unwrap(); }
    // / fn example() -> Result<(), Box<dyn Error>> {
    // /     let mut wtr = WriterBuilder::new().from_writer(vec![]);
    // /     wtr.serialize(("Boston", "United States", 4628910))?;
    // /     wtr.serialize(("Concord", "United States", 42695))?;
    // /
    // /     let data = String::from_utf8(wtr.into_inner()?)?;
    // /     assert_eq!(data, "\
    // / Boston,United States,4628910
    // / Concord,United States,42695
    // / ");
    // /     Ok(())
    // / }
    // / ```
    pub fn has_headers(&mut self, yes: bool) -> &mut AsyncWriterBuilder {
        self.has_headers = yes;
        self
    }

    /// Whether the number of fields in records is allowed to change or not.
    ///
    /// When disabled (which is the default), writing CSV data will return an
    /// error if a record is written with a number of fields different from the
    /// number of fields written in a previous record.
    ///
    /// When enabled, this error checking is turned off.
    ///
    /// # Example: writing flexible records
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::AsyncWriterBuilder;
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .flexible(true)
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "b"]).await?;
    ///     wtr.write_record(&["x", "y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "a,b\nx,y,z\n");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Example: error when `flexible` is disabled
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::AsyncWriterBuilder;
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .flexible(false)
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "b"]).await?;
    ///     let err = wtr.write_record(&["x", "y", "z"]).await.unwrap_err();
    ///     match *err.kind() {
    ///         csv_async::ErrorKind::UnequalLengths { expected_len, len, .. } => {
    ///             assert_eq!(expected_len, 2);
    ///             assert_eq!(len, 3);
    ///         }
    ///         ref wrong => {
    ///             panic!("expected UnequalLengths but got {:?}", wrong);
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn flexible(&mut self, yes: bool) -> &mut AsyncWriterBuilder {
        self.flexible = yes;
        self
    }

    /// The record terminator to use when writing CSV.
    ///
    /// A record terminator can be any single byte. The default is `\n`.
    ///
    /// Note that RFC 4180 specifies that record terminators should be `\r\n`.
    /// To use `\r\n`, use the special `Terminator::CRLF` value.
    ///
    /// # Example: CRLF
    ///
    /// This shows how to use RFC 4180 compliant record terminators.
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::{Terminator, AsyncWriterBuilder};
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .terminator(Terminator::CRLF)
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "b", "c"]).await?;
    ///     wtr.write_record(&["x", "y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "a,b,c\r\nx,y,z\r\n");
    ///     Ok(())
    /// }
    /// ```
    pub fn terminator(&mut self, term: Terminator) -> &mut AsyncWriterBuilder {
        self.builder.terminator(term.to_core());
        self
    }

    /// The quoting style to use when writing CSV.
    ///
    /// By default, this is set to `QuoteStyle::Necessary`, which will only
    /// use quotes when they are necessary to preserve the integrity of data.
    ///
    /// Note that unless the quote style is set to `Never`, an empty field is
    /// quoted if it is the only field in a record.
    ///
    /// # Example: non-numeric quoting
    ///
    /// This shows how to quote non-numeric fields only.
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::{QuoteStyle, AsyncWriterBuilder};
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .quote_style(QuoteStyle::NonNumeric)
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "5", "c"]).await?;
    ///     wtr.write_record(&["3.14", "y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "\"a\",5,\"c\"\n3.14,\"y\",\"z\"\n");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Example: never quote
    ///
    /// This shows how the CSV writer can be made to never write quotes, even
    /// if it sacrifices the integrity of the data.
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::{QuoteStyle, AsyncWriterBuilder};
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .quote_style(QuoteStyle::Never)
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "foo\nbar", "c"]).await?;
    ///     wtr.write_record(&["g\"h\"i", "y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "a,foo\nbar,c\ng\"h\"i,y,z\n");
    ///     Ok(())
    /// }
    /// ```
    pub fn quote_style(&mut self, style: QuoteStyle) -> &mut AsyncWriterBuilder {
        self.builder.quote_style(style.to_core());
        self
    }

    /// The quote character to use when writing CSV.
    ///
    /// The default is `b'"'`.
    ///
    /// # Example
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::AsyncWriterBuilder;
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .quote(b'\'')
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "foo\nbar", "c"]).await?;
    ///     wtr.write_record(&["g'h'i", "y\"y\"y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "a,'foo\nbar',c\n'g''h''i',y\"y\"y,z\n");
    ///     Ok(())
    /// }
    /// ```
    pub fn quote(&mut self, quote: u8) -> &mut AsyncWriterBuilder {
        self.builder.quote(quote);
        self
    }

    /// Enable double quote escapes.
    ///
    /// This is enabled by default, but it may be disabled. When disabled,
    /// quotes in field data are escaped instead of doubled.
    ///
    /// # Example
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::AsyncWriterBuilder;
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .double_quote(false)
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "foo\"bar", "c"]).await?;
    ///     wtr.write_record(&["x", "y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "a,\"foo\\\"bar\",c\nx,y,z\n");
    ///     Ok(())
    /// }
    /// ```
    pub fn double_quote(&mut self, yes: bool) -> &mut AsyncWriterBuilder {
        self.builder.double_quote(yes);
        self
    }

    /// The escape character to use when writing CSV.
    ///
    /// In some variants of CSV, quotes are escaped using a special escape
    /// character like `\` (instead of escaping quotes by doubling them).
    ///
    /// By default, writing these idiosyncratic escapes is disabled, and is
    /// only used when `double_quote` is disabled.
    ///
    /// # Example
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::AsyncWriterBuilder;
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr = AsyncWriterBuilder::new()
    ///         .double_quote(false)
    ///         .escape(b'$')
    ///         .create_writer(vec![]);
    ///     wtr.write_record(&["a", "foo\"bar", "c"]).await?;
    ///     wtr.write_record(&["x", "y", "z"]).await?;
    ///
    ///     let data = String::from_utf8(wtr.into_inner().await?)?;
    ///     assert_eq!(data, "a,\"foo$\"bar\",c\nx,y,z\n");
    ///     Ok(())
    /// }
    /// ```
    pub fn escape(&mut self, escape: u8) -> &mut AsyncWriterBuilder {
        self.builder.escape(escape);
        self
    }

    /// Use this when you are going to set comment for reader used to read saved file.
    ///
    /// If `quote_style` is set to `QuoteStyle::Necessary`, a field will
    /// be quoted if the comment character is detected anywhere in the field.
    ///
    /// The default value is None.
    ///
    /// # Example
    ///
    /// ```
    /// use std::error::Error;
    /// use csv_async::AsyncWriterBuilder;
    ///
    /// # fn main() { async_std::task::block_on(async {example().await.unwrap()}); }
    /// async fn example() -> Result<(), Box<dyn Error>> {
    ///     let mut wtr =
    ///         AsyncWriterBuilder::new().comment(Some(b'#')).create_writer(Vec::new());
    ///     wtr.write_record(&["# comment", "another"]).await?;
    ///     let buf = wtr.into_inner().await?;
    ///     assert_eq!(String::from_utf8(buf).unwrap(), "\"# comment\",another\n");
    ///     Ok(())
    /// }
    /// ```
    pub fn comment(&mut self, comment: Option<u8>) -> &mut AsyncWriterBuilder {
        self.builder.comment(comment);
        self
    }

    /// Set the capacity (in bytes) of the internal buffer used in the CSV
    /// writer. This defaults to a reasonable setting.
    pub fn buffer_capacity(&mut self, capacity: usize) -> &mut AsyncWriterBuilder {
        self.capacity = capacity;
        self
    }
}

//-//////////////////////////////////////////////////////////////////////////////////////////////
//-// Writer
//-//////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
struct WriterState {
    /// Whether inconsistent record lengths are allowed.
    flexible: bool,
    /// The number of fields writtein in the first record. This is compared
    /// with `fields_written` on all subsequent records to check for
    /// inconsistent record lengths.
    first_field_count: Option<u64>,
    /// The number of fields written in this record. This is used to report
    /// errors for inconsistent record lengths if `flexible` is disabled.
    fields_written: u64,
    /// This is set immediately before flushing the buffer and then unset
    /// immediately after flushing the buffer. This avoids flushing the buffer
    /// twice if the inner writer panics.
    panicked: bool,
}

/// A simple internal buffer for buffering writes.
///
/// We need this because the `csv_core` APIs want to write into a `&mut [u8]`,
/// which is not available with the `std::io::BufWriter` API.
#[derive(Debug)]
struct Buffer {
    /// The contents of the buffer.
    buf: Vec<u8>,
    /// The number of bytes written to the buffer.
    len: usize,
}

impl Buffer {
    /// Returns a slice of the buffer's current contents.
    ///
    /// The slice returned may be empty.
    #[inline]
    fn readable(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Returns a mutable slice of the remaining space in this buffer.
    ///
    /// The slice returned may be empty.
    #[inline]
    fn writable(&mut self) -> &mut [u8] {
        &mut self.buf[self.len..]
    }

    /// Indicates that `n` bytes have been written to this buffer.
    #[inline]
    fn written(&mut self, n: usize) {
        self.len += n;
    }

    /// Clear the buffer.
    #[inline]
    fn clear(&mut self) {
        self.len = 0;
    }
}

/// CSV async writer internal implementation used by both record writer and serializer.
/// 
#[derive(Debug)]
pub struct AsyncWriterImpl<W: AsyncWrite + Unpin> {
    core: CoreWriter,
    wtr: Option<W>,
    buf: Buffer,
    state: WriterState,
}

impl<W: AsyncWrite + Unpin> Drop for AsyncWriterImpl<W> {
    fn drop(&mut self) {
        if self.wtr.is_some() && !self.state.panicked {
            // We ignore result of flush() call while dropping
            // Well known problem.
            // If you care about flush result call it explicitly 
            // before AsyncWriter goes out of scope,
            // second flush() call should be no op.
            let _ = futures::executor::block_on(self.flush());
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWriterImpl<W> {
    fn new(builder: &AsyncWriterBuilder, wtr: W) -> AsyncWriterImpl<W> {
        AsyncWriterImpl {
            core: builder.builder.build(),
            wtr: Some(wtr),
            buf: Buffer { buf: vec![0; builder.capacity], len: 0 },
            state: WriterState {
                flexible: builder.flexible,
                first_field_count: None,
                fields_written: 0,
                panicked: false,
            },
        }
    }

    /// Write a single record.
    ///
    pub async fn write_record<I, T>(&mut self, record: I) -> Result<()>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<[u8]>,
    {
        for field in record.into_iter() {
            self.write_field_impl(field).await?;
        }
        self.write_terminator().await
    }

    /// Write a single `ByteRecord`.
    ///
    #[inline(never)]
    pub async fn write_byte_record(&mut self, record: &ByteRecord) -> Result<()> {
        if record.as_slice().is_empty() {
            return self.write_record(record).await;
        }
        // The idea here is to find a fast path for shuffling our record into
        // our buffer as quickly as possible. We do this because the underlying
        // "core" CSV writer does a lot of book-keeping to maintain its state
        // oriented API.
        //
        // The fast path occurs when we know our record will fit in whatever
        // space we have left in our buffer. We can actually quickly compute
        // the upper bound on the space required:
        let upper_bound =
            // The data itself plus the worst case: every byte is a quote.
            (2 * record.as_slice().len())
            // The number of field delimiters.
            + (record.len().saturating_sub(1))
            // The maximum number of quotes inserted around each field.
            + (2 * record.len())
            // The maximum number of bytes for the terminator.
            + 2;
        if self.buf.writable().len() < upper_bound {
            return self.write_record(record).await;
        }
        let mut first = true;
        for field in record.iter() {
            if !first {
                self.buf.writable()[0] = self.core.get_delimiter();
                self.buf.written(1);
            }
            first = false;

            if !self.core.should_quote(field) {
                self.buf.writable()[..field.len()].copy_from_slice(field);
                self.buf.written(field.len());
            } else {
                self.buf.writable()[0] = self.core.get_quote();
                self.buf.written(1);
                let (res, nin, nout) = csv_core::quote(
                    field,
                    self.buf.writable(),
                    self.core.get_quote(),
                    self.core.get_escape(),
                    self.core.get_double_quote(),
                );
                debug_assert!(res == WriteResult::InputEmpty);
                debug_assert!(nin == field.len());
                self.buf.written(nout);
                self.buf.writable()[0] = self.core.get_quote();
                self.buf.written(1);
            }
        }
        self.state.fields_written = record.len() as u64;
        self.write_terminator_into_buffer()
    }

    /// Write a single field.
    ///
    pub async fn write_field<T: AsRef<[u8]>>(&mut self, field: T) -> Result<()> {
        self.write_field_impl(field).await
    }

    /// Implementation of write_field.
    ///
    /// This is a separate method so we can force the compiler to inline it
    /// into write_record.
    #[inline(always)]
    async fn write_field_impl<T: AsRef<[u8]>>(&mut self, field: T) -> Result<()> {
        if self.state.fields_written > 0 {
            self.write_delimiter().await?;
        }
        let mut field = field.as_ref();
        loop {
            let (res, nin, nout) = self.core.field(field, self.buf.writable());
            field = &field[nin..];
            self.buf.written(nout);
            match res {
                WriteResult::InputEmpty => {
                    self.state.fields_written += 1;
                    return Ok(());
                }
                WriteResult::OutputFull => self.flush_buf().await?,
            }
        }
    }

    /// Flush the contents of the internal buffer to the underlying writer.
    ///
    /// If there was a problem writing to the underlying writer, then an error
    /// is returned.
    ///
    /// Note that this also flushes the underlying writer.
    pub async fn flush(&mut self) -> io::Result<()> {
        self.flush_buf().await?;
        self.wtr.as_mut().unwrap().flush().await?;
        Ok(())
    }

    /// Flush the contents of the internal buffer to the underlying writer,
    /// without flushing the underlying writer.
    async fn flush_buf(&mut self) -> io::Result<()> {
        self.state.panicked = true;
        let result = self.wtr.as_mut().unwrap().write_all(self.buf.readable()).await;
        self.state.panicked = false;
        result?;
        self.buf.clear();
        Ok(())
    }

    /// Flush the contents of the internal buffer and return the underlying
    /// writer.
    pub async fn into_inner(
        mut self,
    ) -> result::Result<W, IntoInnerError<AsyncWriterImpl<W>>> {
        match self.flush().await {
            Ok(()) => Ok(self.wtr.take().unwrap()),
            Err(err) => Err(IntoInnerError::new(self, err)),
        }
    }

    /// Write a CSV delimiter.
    async fn write_delimiter(&mut self) -> Result<()> {
        loop {
            let (res, nout) = self.core.delimiter(self.buf.writable());
            self.buf.written(nout);
            match res {
                WriteResult::InputEmpty => return Ok(()),
                WriteResult::OutputFull => self.flush_buf().await?,
            }
        }
    }

    /// Write a CSV terminator.
    async fn write_terminator(&mut self) -> Result<()> {
        self.check_field_count()?;
        loop {
            let (res, nout) = self.core.terminator(self.buf.writable());
            self.buf.written(nout);
            match res {
                WriteResult::InputEmpty => {
                    self.state.fields_written = 0;
                    return Ok(());
                }
                WriteResult::OutputFull => self.flush_buf().await?,
            }
        }
    }

    /// Write a CSV terminator that is guaranteed to fit into the current buffer.
    /// 
    #[inline(never)]
    fn write_terminator_into_buffer(&mut self) -> Result<()> {
        self.check_field_count()?;
        match self.core.get_terminator() {
            csv_core::Terminator::CRLF => {
                self.buf.writable()[0] = b'\r';
                self.buf.writable()[1] = b'\n';
                self.buf.written(2);
            }
            csv_core::Terminator::Any(b) => {
                self.buf.writable()[0] = b;
                self.buf.written(1);
            }
            _ => unreachable!(),
        }
        self.state.fields_written = 0;
        Ok(())
    }

    fn check_field_count(&mut self) -> Result<()> {
        if !self.state.flexible {
            match self.state.first_field_count {
                None => {
                    self.state.first_field_count =
                        Some(self.state.fields_written);
                }
                Some(expected) if expected != self.state.fields_written => {
                    return Err(Error::new(ErrorKind::UnequalLengths {
                        pos: None,
                        expected_len: expected,
                        len: self.state.fields_written,
                    }))
                }
                Some(_) => {}
            }
        }
        Ok(())
    }
}
