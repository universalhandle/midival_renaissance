use crate::configuration::{
    Config, EnvelopeTrigger, InputMode, InstrumentConfig, NoteEmbargo, NotePriority,
};

// maybe the essential configs are type parameters, so that we impl TraitX
// differently for Micromoog<InputMode=Keyboard> vs Micromoog<InputMode=Oscillator>?
pub struct Micromoog {
    config: InstrumentConfig,
}

impl Micromoog {
    fn new(config: InstrumentConfig) -> Self {
        Self { config }
    }
}

impl Default for Micromoog {
    fn default() -> Self {
        Self::new(InstrumentConfig {
            envelope_trigger: EnvelopeTrigger::BreakEnd,
            input_mode: InputMode::default(),
            note_embargo: NoteEmbargo::None,
            note_priority: NotePriority::Low,
        })
    }
}

impl Config for Micromoog {
    fn config(&self) -> &InstrumentConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut InstrumentConfig {
        &mut self.config
    }
}
