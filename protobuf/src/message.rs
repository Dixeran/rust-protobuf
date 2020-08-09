use std::any::Any;
use std::any::TypeId;
use std::fmt;
use std::io::Read;
use std::io::Write;

#[cfg(feature = "bytes")]
use bytes::Bytes;

use crate::clear::Clear;
use crate::error::ProtobufError;
use crate::error::ProtobufResult;
use crate::reflect::reflect_eq::ReflectEqMode;
use crate::reflect::MessageDescriptor;
use crate::reflect::ProtobufValue;
use crate::stream::CodedInputStream;
use crate::stream::CodedOutputStream;
use crate::stream::WithCodedOutputStream;
use crate::unknown::UnknownFields;

/// Trait implemented for all generated structs for protobuf messages.
///
/// Also, generated messages implement `Clone + Default + PartialEq`
pub trait Message: fmt::Debug + Clear + Send + Sync + ProtobufValue {
    /// Message descriptor for this message, used for reflection.
    fn descriptor(&self) -> &'static MessageDescriptor {
        unimplemented!()
    }

    /// Get message descriptor for message type.
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>() {
    /// let descriptor = MyMessage::descriptor_static();
    /// assert_eq!("MyMessage", descriptor.name());
    /// # }
    /// ```
    fn descriptor_static() -> &'static MessageDescriptor
    where
        Self: Sized,
    {
        panic!(
            "descriptor_static is not implemented for message, \
             LITE_RUNTIME must be used"
        );
    }

    /// TODO
    fn descriptor_new(&self) -> MessageDescriptor {
        unimplemented!();
    }

    /// TODO
    fn descriptor_static_new() -> MessageDescriptor
    where
        Self: Sized,
    {
        unimplemented!();
    }

    /// True iff all required fields are initialized.
    /// Always returns `true` for protobuf 3.
    fn is_initialized(&self) -> bool;

    /// Update this message object with fields read from given stream.
    fn merge_from(&mut self, is: &mut CodedInputStream) -> ProtobufResult<()>;

    /// Parse message from stream.
    fn parse_from(is: &mut CodedInputStream) -> ProtobufResult<Self>
    where
        Self: Sized,
    {
        let mut r: Self = Message::new();
        r.merge_from(is)?;
        r.check_initialized()?;
        Ok(r)
    }

    /// Write message to the stream.
    ///
    /// Sizes of this messages and nested messages must be cached
    /// by calling `compute_size` prior to this call.
    fn write_to_with_cached_sizes(&self, os: &mut CodedOutputStream) -> ProtobufResult<()>;

    /// Compute and cache size of this message and all nested messages
    fn compute_size(&self) -> u32;

    /// Get size previously computed by `compute_size`.
    fn get_cached_size(&self) -> u32;

    /// Write the message to the stream.
    ///
    /// Results in error if message is not fully initialized.
    fn write_to(&self, os: &mut CodedOutputStream) -> ProtobufResult<()> {
        self.check_initialized()?;

        // cache sizes
        self.compute_size();
        // TODO: reserve additional
        self.write_to_with_cached_sizes(os)?;

        Ok(())
    }

    /// Write the message to the stream prepending the message with message length
    /// encoded as varint.
    fn write_length_delimited_to(&self, os: &mut CodedOutputStream) -> ProtobufResult<()> {
        let size = self.compute_size();
        os.write_raw_varint32(size)?;
        self.write_to_with_cached_sizes(os)?;

        // TODO: assert we've written same number of bytes as computed

        Ok(())
    }

    /// Write the message to the vec, prepend the message with message length
    /// encoded as varint.
    fn write_length_delimited_to_vec(&self, vec: &mut Vec<u8>) -> ProtobufResult<()> {
        let mut os = CodedOutputStream::vec(vec);
        self.write_length_delimited_to(&mut os)?;
        os.flush()?;
        Ok(())
    }

    /// Update this message object with fields read from given stream.
    fn merge_from_bytes(&mut self, bytes: &[u8]) -> ProtobufResult<()> {
        let mut is = CodedInputStream::from_bytes(bytes);
        self.merge_from(&mut is)
    }

    /// Parse message from reader.
    /// Parse stops on EOF or when error encountered.
    fn parse_from_reader(reader: &mut dyn Read) -> ProtobufResult<Self>
    where
        Self: Sized,
    {
        let mut is = CodedInputStream::new(reader);
        let r = Message::parse_from(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }

    /// Parse message from byte array.
    fn parse_from_bytes(bytes: &[u8]) -> ProtobufResult<Self>
    where
        Self: Sized,
    {
        let mut is = CodedInputStream::from_bytes(bytes);
        let r = Message::parse_from(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }

    /// Parse message from `Bytes` object.
    /// Resulting message may share references to the passed bytes object.
    #[cfg(feature = "bytes")]
    fn parse_from_carllerche_bytes(bytes: &Bytes) -> ProtobufResult<Self>
    where
        Self: Sized,
    {
        let mut is = CodedInputStream::from_carllerche_bytes(bytes);
        let r = Self::parse_from(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }

    /// Check if all required fields of this object are initialized.
    fn check_initialized(&self) -> ProtobufResult<()> {
        if !self.is_initialized() {
            Err(ProtobufError::MessageNotInitialized(
                self.descriptor().name(),
            ))
        } else {
            Ok(())
        }
    }

    /// Write the message to the writer.
    fn write_to_writer(&self, w: &mut dyn Write) -> ProtobufResult<()> {
        w.with_coded_output_stream(|os| self.write_to(os))
    }

    /// Write the message to bytes vec.
    fn write_to_vec(&self, v: &mut Vec<u8>) -> ProtobufResult<()> {
        v.with_coded_output_stream(|os| self.write_to(os))
    }

    /// Write the message to bytes vec.
    ///    
    /// > **Note**: You can use [`Message::parse_from_bytes`]
    /// to do the reverse.
    fn write_to_bytes(&self) -> ProtobufResult<Vec<u8>> {
        self.check_initialized()?;

        let size = self.compute_size() as usize;
        let mut v = Vec::with_capacity(size);
        // skip zerofill
        unsafe {
            v.set_len(size);
        }
        {
            let mut os = CodedOutputStream::bytes(&mut v);
            self.write_to_with_cached_sizes(&mut os)?;
            os.check_eof();
        }
        Ok(v)
    }

    /// Write the message to the writer, prepend the message with message length
    /// encoded as varint.
    fn write_length_delimited_to_writer(&self, w: &mut dyn Write) -> ProtobufResult<()> {
        w.with_coded_output_stream(|os| self.write_length_delimited_to(os))
    }

    /// Write the message to the bytes vec, prepend the message with message length
    /// encoded as varint.
    fn write_length_delimited_to_bytes(&self) -> ProtobufResult<Vec<u8>> {
        let mut v = Vec::new();
        v.with_coded_output_stream(|os| self.write_length_delimited_to(os))?;
        Ok(v)
    }

    /// Get a reference to unknown fields.
    fn get_unknown_fields(&self) -> &UnknownFields;
    /// Get a mutable reference to unknown fields.
    fn mut_unknown_fields(&mut self) -> &mut UnknownFields;

    /// Create an empty message object.
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>() {
    /// let m = MyMessage::new();
    /// # }
    /// ```
    fn new() -> Self
    where
        Self: Sized;

    /// Return a pointer to default immutable message with static lifetime.
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>() {
    /// let m: &MyMessage = MyMessage::default_instance();
    /// # }
    /// ```
    fn default_instance() -> &'static Self
    where
        Self: Sized;
}

impl dyn Message {
    /// Downcast `Box<dyn Message>` to specific message type.
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>(message: Box<dyn Message>) {
    /// let m: Box<dyn Message> = message;
    /// let m: Box<MyMessage> = Message::downcast_box(m).unwrap();
    /// # }
    /// ```
    pub fn downcast_box<T: Any>(self: Box<dyn Message>) -> Result<Box<T>, Box<dyn Message>> {
        if Any::type_id(&*self) == TypeId::of::<T>() {
            unsafe {
                let raw: *mut dyn Message = Box::into_raw(self);
                Ok(Box::from_raw(raw as *mut T))
            }
        } else {
            Err(self)
        }
    }

    /// Downcast `&dyn Message` to specific message type.
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>(message: &dyn Message) {
    /// let m: &dyn Message = message;
    /// let m: &MyMessage = Message::downcast_ref(m).unwrap();
    /// # }
    /// ```
    pub fn downcast_ref<'a, M: Message + 'a>(&'a self) -> Option<&'a M> {
        if Any::type_id(&*self) == TypeId::of::<M>() {
            unsafe { Some(&*(self as *const dyn Message as *const M)) }
        } else {
            None
        }
    }

    /// Downcast `&mut dyn Message` to specific message type.
    ///
    /// ```
    /// # use protobuf::Message;
    /// # fn foo<MyMessage: Message>(message: &mut dyn Message) {
    /// let m: &mut dyn Message = message;
    /// let m: &mut MyMessage = Message::downcast_mut(m).unwrap();
    /// # }
    /// ```
    pub fn downcast_mut<'a, M: Message + 'a>(&'a mut self) -> Option<&'a mut M> {
        if Any::type_id(&*self) == TypeId::of::<M>() {
            unsafe { Some(&mut *(self as *mut dyn Message as *mut M)) }
        } else {
            None
        }
    }

    /// Check two messages for equality.
    ///
    /// Messages of different types are not equal,
    /// `NaN` values are considered equal (useful for tests).
    pub fn reflect_eq(&self, other: &dyn Message) -> bool {
        let d = self.descriptor();
        if d != other.descriptor() {
            return false;
        }
        d.reflect_eq(self, other, &ReflectEqMode { nan_equal: true })
    }

    /// Clone from a `dyn Message` reference.
    pub fn clone_box(&self) -> Box<dyn Message> {
        self.descriptor().clone(self)
    }
}

impl Clone for Box<dyn Message> {
    fn clone(&self) -> Self {
        (*self).clone_box()
    }
}

#[cfg(off)] // don't need it
impl PartialEq for Box<dyn Message> {
    fn eq(&self, other: &Box<dyn Message>) -> bool {
        use std::ops::Deref;
        self.descriptor() == other.descriptor() && self.descriptor().eq(self.deref(), other.deref())
    }
}
