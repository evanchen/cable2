use crate::{
    message::{SMReceiver, SMReceiverChan, SMSender, SMSenderChan},
    states::GameState,
};
use tokio::sync::mpsc::channel;

#[derive(Default)]
pub struct Module {
    name: String,
    smsender: Option<SMSender>,
    smreceiver: Option<SMReceiver>,
    smsender_chan: Option<SMSenderChan>,
    smreceiver_chan: Option<SMReceiverChan>,
    game_state: Option<GameState>,
}

impl Module {
    pub fn new(module_name: String) -> Self {
        let mut m = Module::default();
        m.name = module_name;
        m
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn with_sender(mut self, sender_size: usize) -> Self {
        let (sender, receiver) = channel(sender_size);
        self.smsender = Some(sender);
        self.smreceiver = Some(receiver);
        self
    }

    pub fn with_sender_chan(mut self, sender_chan_size: usize) -> Self {
        let (sender, receiver) = channel(sender_chan_size);
        self.smsender_chan = Some(sender);
        self.smreceiver_chan = Some(receiver);
        self
    }

    pub fn with_game_state(mut self, gs: GameState) -> Self {
        self.game_state = Some(gs);
        self
    }

    pub fn spawn_smsender(&self) -> SMSender {
        self.smsender.as_ref().unwrap().clone()
    }

    pub fn take_smreceiver(&mut self) -> Option<SMReceiver> {
        self.smreceiver.take()
    }

    pub fn spawn_smsender_chan(&self) -> SMSenderChan {
        self.smsender_chan.as_ref().unwrap().clone()
    }

    pub fn take_smreceiver_chan(&mut self) -> Option<SMReceiverChan> {
        self.smreceiver_chan.take()
    }

    pub fn take_game_state(&mut self) -> Option<GameState> {
        self.game_state.take()
    }

    pub fn get_game_state(&mut self) -> &mut GameState {
        self.game_state.as_mut().unwrap()
    }
}
