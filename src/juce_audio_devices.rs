//! Play and record from audio and MIDI I/O devices.

use {
    crate::{juce, Result, JUCE},
    std::{
        ops::{Index, IndexMut},
        pin::Pin,
    },
};

/// A multi-channel buffer of read-only audio samples.
pub struct InputAudioSampleBuffer<'a> {
    buffer: &'a juce::AudioSampleBuffer,
}

impl<'a> InputAudioSampleBuffer<'a> {
    pub(crate) fn new(buffer: &'a juce::AudioSampleBuffer) -> Self {
        Self { buffer }
    }

    /// Returns the numbers of channels in the buffer.
    pub fn channels(&self) -> usize {
        self.buffer.get_num_channels() as usize
    }

    /// Returns the number of samples for each channel.
    pub fn samples(&self) -> usize {
        self.buffer.get_num_samples() as usize
    }
}

impl Index<usize> for InputAudioSampleBuffer<'_> {
    type Output = [f32];

    fn index(&self, channel: usize) -> &Self::Output {
        if self.channels() < channel {
            panic!("channel out of bounds");
        }

        let ptr = self.buffer.get_read_pointer(channel as i32);
        let len = self.samples();

        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// A multi-channel buffer of read-write audio samples.
pub struct OutputAudioSampleBuffer<'a> {
    buffer: Pin<&'a mut juce::AudioSampleBuffer>,
}

impl<'a> OutputAudioSampleBuffer<'a> {
    pub(crate) fn new(buffer: Pin<&'a mut juce::AudioSampleBuffer>) -> Self {
        Self { buffer }
    }

    /// Returns the numbers of channels in the buffer.
    pub fn channels(&self) -> usize {
        self.buffer.get_num_channels() as usize
    }

    /// Returns the number of samples for each channel.
    pub fn samples(&self) -> usize {
        self.buffer.get_num_samples() as usize
    }

    /// Clear all the samples for all the channels.
    pub fn clear(&mut self) {
        self.buffer.as_mut().clear();
    }
}

impl Index<usize> for OutputAudioSampleBuffer<'_> {
    type Output = [f32];

    fn index(&self, channel: usize) -> &Self::Output {
        if self.channels() < channel {
            panic!("channel out of bounds");
        }

        let ptr = self.buffer.get_read_pointer(channel as i32);
        let len = self.samples();

        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

impl IndexMut<usize> for OutputAudioSampleBuffer<'_> {
    fn index_mut(&mut self, channel: usize) -> &mut Self::Output {
        if self.channels() < channel {
            panic!("channel out of bounds");
        }

        let ptr = self.buffer.as_mut().get_write_pointer(channel as i32);
        let len = self.samples();

        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }
}

/// The properties of an audio device.
pub struct AudioDeviceSetup(cxx::UniquePtr<juce::AudioDeviceSetup>);

impl Default for AudioDeviceSetup {
    fn default() -> Self {
        Self(juce::create_audio_device_setup())
    }
}

impl AudioDeviceSetup {
    /// The name of the output device.
    pub fn output_device_name(&self) -> &str {
        self.0.output_device_name()
    }

    /// Set the name of the output device.
    pub fn with_output_device_name(mut self, name: impl AsRef<str>) -> Self {
        self.0.pin_mut().set_output_device_name(name.as_ref());
        self
    }

    /// The name of the input device.
    pub fn input_device_name(&self) -> &str {
        self.0.input_device_name()
    }

    /// Set the name of the input device.
    pub fn with_input_device_name(mut self, name: impl AsRef<str>) -> Self {
        self.0.pin_mut().set_input_device_name(name.as_ref());
        self
    }

    /// The sample rate in Hertz.
    pub fn sample_rate(&self) -> f64 {
        self.0.sample_rate()
    }

    /// Set the sample rate in Hertz.
    pub fn with_sample_rate(mut self, sample_rate: f64) -> Self {
        self.0.pin_mut().set_sample_rate(sample_rate);
        self
    }

    /// The buffer size.
    pub fn buffer_size(&self) -> usize {
        self.0.buffer_size() as usize
    }

    /// The buffer size to use.
    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.0.pin_mut().set_buffer_size(buffer_size as i32);
        self
    }
}

/// Manages the state of an audio device.
pub struct AudioDeviceManager {
    device_manager: cxx::UniquePtr<juce::AudioDeviceManager>,
    _juce: JUCE,
}

impl Default for AudioDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDeviceManager {
    /// Create a new [`AudioDeviceManager`].
    pub fn new() -> Self {
        let juce = JUCE::initialise();

        Self {
            device_manager: juce::create_audio_device_manager(),
            _juce: juce,
        }
    }

    /// Resets to a default device setup.
    pub fn initialise(&mut self, input_channels: usize, output_channels: usize) -> Result<()> {
        self.device_manager
            .pin_mut()
            .initialise_with_default_devices(input_channels as i32, output_channels as i32)
    }

    /// Get the current device setup.
    pub fn audio_device_setup(&self) -> AudioDeviceSetup {
        AudioDeviceSetup(self.device_manager.get_audio_device_setup())
    }

    /// Changes the current device or its settings.
    pub fn set_audio_device_setup(&mut self, setup: &AudioDeviceSetup) {
        self.device_manager
            .pin_mut()
            .set_audio_device_setup(&setup.0);
    }

    /// Play a test sound.
    pub fn play_test_sound(&mut self) {
        self.device_manager.pin_mut().play_test_sound();
    }

    /// Get the available device types.
    pub fn device_types(&mut self) -> Vec<impl AudioIODeviceType + '_> {
        let available_device_types = self.device_manager.pin_mut().get_available_device_types();

        (0..available_device_types.size())
            .map(|i| available_device_types.get_unchecked(i))
            .collect()
    }

    /// Get the current device type.
    pub fn current_device_type(&self) -> impl AudioIODeviceType + '_ {
        self.device_manager.get_current_device_type_object()
    }

    /// Get the current [`AudioIODevice`].
    pub fn current_device(&self) -> impl AudioIODevice + '_ {
        self.device_manager.get_current_audio_device()
    }

    /// Registers an audio callback.
    ///
    /// When the returned [`AudioCallbackHandle`] is dropped the callback is removed.
    pub fn add_audio_callback(
        &mut self,
        callback: impl AudioIODeviceCallback + 'static,
    ) -> AudioCallbackHandle<'_> {
        let callback = Box::new(callback);

        AudioCallbackHandle(
            self.device_manager
                .pin_mut()
                .add_audio_callback(Box::new(callback)),
        )
    }

    /// Registers an audio device type.
    pub fn add_audio_device_type(&mut self, device_type: impl AudioIODeviceType + 'static) {
        let device_type = Box::new(device_type);
        self.device_manager
            .pin_mut()
            .add_audio_device_type(Box::new(device_type));
    }

    /// Set the current audio device type to use.
    pub fn set_current_audio_device_type(&mut self, device_type: &str) {
        self.device_manager
            .pin_mut()
            .set_current_audio_device_type(device_type);
    }
}

/// A trait that can be implemented to receive audio callbacks.
///
/// Types that implement this trait can be registered with [`AudioDeviceManager::add_audio_callback`].
///
/// This trait requires that implementors are [`Send`] because the callbacks will occur on the audio thread.
pub trait AudioIODeviceCallback: Send {
    /// Called when the audio device is about to start.
    fn about_to_start(
        &mut self,
        input_channels: usize,
        output_channels: usize,
        sample_rate: f64,
        buffer_size: usize,
    );

    /// Process a block of incoming and outgoing audio.
    fn process_block(
        &mut self,
        input: &InputAudioSampleBuffer<'_>,
        output: &mut OutputAudioSampleBuffer<'_>,
    );

    /// Called when the audio device has stopped.
    fn stopped(&mut self);
}

pub(crate) type BoxedAudioIODeviceCallback = Box<dyn AudioIODeviceCallback>;
pub(crate) type BoxedAudioIODeviceType = Box<dyn AudioIODeviceType>;
pub(crate) type BoxedAudioIODevice = Box<dyn AudioIODevice>;

/// A handle to a registered audio callback.
///
/// When this handle is dropped the callback is removed.
#[must_use]
pub struct AudioCallbackHandle<'a>(cxx::UniquePtr<juce::AudioCallbackHandle<'a>>);

/// A trait representing a type of audio driver (e.g. CoreAudio, ASIO, etc.).
pub trait AudioIODeviceType {
    /// The name of the type of driver.
    fn name(&self) -> String;

    /// Refreshes the drivers cached list of known devices.
    fn scan_for_devices(&mut self);

    /// Returns a list of known input devices.
    fn input_devices(&self) -> Vec<String>;

    /// Returns a list of the known output devices.
    fn output_devices(&self) -> Vec<String>;

    /// Create an [`AudioIODevice`].
    fn create_device(
        &mut self,
        input_device_name: &str,
        output_device_name: &str,
    ) -> Option<Box<dyn AudioIODevice>>;
}

impl AudioIODeviceType for *mut juce::AudioIODeviceType {
    fn name(&self) -> String {
        if self.is_null() {
            return String::default();
        }

        let this = unsafe { &*self.cast_const() };
        juce::get_type_name(this)
    }

    fn scan_for_devices(&mut self) {
        if let Some(this) = unsafe { self.as_mut().map(|ptr| Pin::new_unchecked(ptr)) } {
            this.scan_for_devices();
        }
    }

    fn input_devices(&self) -> Vec<String> {
        if self.is_null() {
            return vec![];
        }

        let this = unsafe { &*self.cast_const() };
        juce::get_input_device_names(this)
    }

    fn output_devices(&self) -> Vec<String> {
        if self.is_null() {
            return vec![];
        }

        let this = unsafe { &*self.cast_const() };
        juce::get_output_device_names(this)
    }

    fn create_device(
        &mut self,
        input_device_name: &str,
        output_device_name: &str,
    ) -> Option<Box<dyn AudioIODevice>> {
        unsafe { self.as_mut().map(|ptr| Pin::new_unchecked(ptr)) }
            .map(|this| juce::new_device(this, input_device_name, output_device_name))
            .filter(|device| !device.is_null())
            .map(|device| Box::new(device) as _)
    }
}

/// A trait representing an audio device.
pub trait AudioIODevice {
    /// The name of the device.
    fn name(&self) -> &str;

    /// The type of the device.
    fn type_name(&self) -> &str;

    /// The current sample rate.
    fn sample_rate(&mut self) -> f64;

    /// The current buffer size.
    fn buffer_size(&mut self) -> usize;

    /// The available sample rates.
    fn available_sample_rates(&mut self) -> Vec<f64>;

    /// The available buffer sizes.
    fn available_buffer_sizes(&mut self) -> Vec<usize>;

    /// Tries to open the device so that it can be used for audio processing.
    fn open(&mut self, sample_rate: f64, buffer_size: usize) -> Result<()>;

    /// Close the device.
    fn close(&mut self);
}

impl AudioIODevice for *mut juce::AudioIODevice {
    fn name(&self) -> &str {
        unsafe { self.as_ref() }
            .map(juce::get_device_name)
            .unwrap_or_default()
    }

    fn type_name(&self) -> &str {
        unsafe { self.as_ref() }
            .map(juce::get_device_type_name)
            .unwrap_or_default()
    }

    fn sample_rate(&mut self) -> f64 {
        unsafe { self.as_mut().map(|this| Pin::new_unchecked(this)) }
            .map(|this| this.get_current_sample_rate())
            .unwrap_or_default()
    }

    fn buffer_size(&mut self) -> usize {
        unsafe { self.as_mut().map(|this| Pin::new_unchecked(this)) }
            .map(|this| this.get_current_buffer_size_samples() as usize)
            .unwrap_or_default()
    }

    fn available_sample_rates(&mut self) -> Vec<f64> {
        unsafe { self.as_mut().map(|this| Pin::new_unchecked(this)) }
            .map(juce::get_available_sample_rates)
            .unwrap_or_default()
    }

    fn available_buffer_sizes(&mut self) -> Vec<usize> {
        unsafe { self.as_mut().map(|this| Pin::new_unchecked(this)) }
            .map(juce::get_available_buffer_sizes)
            .unwrap_or_default()
    }

    fn open(&mut self, sample_rate: f64, buffer_size: usize) -> Result<()> {
        if let Some(this) = unsafe { self.as_mut().map(|this| Pin::new_unchecked(this)) } {
            juce::open(this, sample_rate, buffer_size)?;
        }

        Ok(())
    }

    fn close(&mut self) {
        if let Some(this) = unsafe { self.as_mut().map(|this| Pin::new_unchecked(this)) } {
            this.close();
        }
    }
}

impl AudioIODevice for cxx::UniquePtr<juce::AudioIODevice> {
    fn name(&self) -> &str {
        self.as_ref().map(juce::get_device_name).unwrap_or_default()
    }

    fn type_name(&self) -> &str {
        self.as_ref()
            .map(juce::get_device_type_name)
            .unwrap_or_default()
    }

    fn sample_rate(&mut self) -> f64 {
        self.as_mut()
            .map(|this| this.get_current_sample_rate())
            .unwrap_or_default()
    }

    fn buffer_size(&mut self) -> usize {
        self.as_mut()
            .map(|this| this.get_current_buffer_size_samples() as usize)
            .unwrap_or_default()
    }

    fn available_sample_rates(&mut self) -> Vec<f64> {
        self.as_mut()
            .map(juce::get_available_sample_rates)
            .unwrap_or_default()
    }

    fn available_buffer_sizes(&mut self) -> Vec<usize> {
        self.as_mut()
            .map(juce::get_available_buffer_sizes)
            .unwrap_or_default()
    }

    fn open(&mut self, sample_rate: f64, buffer_size: usize) -> Result<()> {
        if let Some(this) = self.as_mut() {
            juce::open(this, sample_rate, buffer_size)?;
        }

        Ok(())
    }

    fn close(&mut self) {
        if let Some(this) = self.as_mut() {
            this.close();
        }
    }
}

pub(crate) mod ffi {
    use super::*;

    pub mod audio_io_device_callback {
        use super::*;

        pub fn about_to_start(
            mut self_: Pin<&mut BoxedAudioIODeviceCallback>,
            mut device: Pin<&mut juce::AudioIODevice>,
        ) {
            let input_channels = juce::count_active_input_channels(&device);
            let output_channels = juce::count_active_output_channels(&device);

            self_.about_to_start(
                input_channels,
                output_channels,
                device.as_mut().get_current_sample_rate(),
                device.as_mut().get_current_buffer_size_samples() as usize,
            );
        }

        pub fn process_block(
            mut self_: Pin<&mut BoxedAudioIODeviceCallback>,
            input: &juce::AudioSampleBuffer,
            output: Pin<&mut juce::AudioSampleBuffer>,
        ) {
            let input = InputAudioSampleBuffer::new(input);
            let mut output = OutputAudioSampleBuffer::new(output);

            self_.process_block(&input, &mut output);
        }

        pub fn stopped(mut self_: Pin<&mut BoxedAudioIODeviceCallback>) {
            self_.stopped()
        }
    }

    pub mod audio_io_device_type {
        use {super::*, std::ptr::null_mut};

        pub fn name(self_: &BoxedAudioIODeviceType) -> String {
            self_.name()
        }

        pub fn scan_for_devices(mut self_: Pin<&mut BoxedAudioIODeviceType>) {
            self_.scan_for_devices()
        }

        pub fn get_device_names(self_: &BoxedAudioIODeviceType, input: bool) -> Vec<String> {
            if input {
                self_.input_devices()
            } else {
                self_.output_devices()
            }
        }

        pub fn create_device(
            mut self_: Pin<&mut BoxedAudioIODeviceType>,
            input_name: &str,
            output_name: &str,
        ) -> *mut BoxedAudioIODevice {
            let device = self_.as_mut().create_device(input_name, output_name);

            device
                .map(|device| Box::into_raw(Box::new(device)))
                .unwrap_or(null_mut())
        }

        pub fn destroy_device(device: *mut BoxedAudioIODevice) {
            if device.is_null() {
                return;
            }

            unsafe { Box::from_raw(device) };
        }
    }

    pub mod audio_io_device {
        use super::*;

        pub fn device_name(self_: &BoxedAudioIODevice) -> String {
            self_.name().to_string()
        }

        pub fn device_type_name(self_: &BoxedAudioIODevice) -> String {
            self_.type_name().to_string()
        }

        pub fn device_sample_rate(mut self_: Pin<&mut BoxedAudioIODevice>) -> f64 {
            self_.sample_rate()
        }

        pub fn device_buffer_size(mut self_: Pin<&mut BoxedAudioIODevice>) -> usize {
            self_.buffer_size()
        }

        pub fn device_available_sample_rates(mut self_: Pin<&mut BoxedAudioIODevice>) -> Vec<f64> {
            self_.available_sample_rates()
        }

        pub fn device_available_buffer_sizes(
            mut self_: Pin<&mut BoxedAudioIODevice>,
        ) -> Vec<usize> {
            self_.available_buffer_sizes()
        }

        pub fn device_open(
            mut self_: Pin<&mut BoxedAudioIODevice>,
            sample_rate: f64,
            buffer_size: usize,
        ) -> String {
            match self_.open(sample_rate, buffer_size) {
                Ok(()) => String::default(),
                Err(error) => error.to_string(),
            }
        }

        pub fn device_close(mut self_: Pin<&mut BoxedAudioIODevice>) {
            self_.close()
        }
    }
}

/// Controls for the system volume.
pub struct SystemAudioVolume;

impl SystemAudioVolume {
    /// Get the current system volume.
    pub fn get_gain() -> f32 {
        juce::get_gain()
    }

    /// Set the system volume.
    pub fn set_gain(gain: f32) {
        juce::set_gain(gain.max(0.0).min(1.0))
    }

    /// Returns true if the system audio output is muted.
    pub fn is_muted() -> bool {
        juce::is_muted()
    }

    /// Mute the system audio output.
    pub fn mute() {
        juce::set_muted(true);
    }

    /// Unmute the system audio output.
    pub fn unmute() {
        juce::set_muted(false);
    }
}
