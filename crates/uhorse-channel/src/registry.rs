use std::collections::HashMap;
use std::sync::Arc;

use uhorse_core::{Channel, ChannelCapabilityFlags, ChannelType};

use crate::DingTalkChannel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredChannel {
    pub channel_type: ChannelType,
    pub capability_flags: ChannelCapabilityFlags,
}

#[derive(Debug, Default, Clone)]
pub struct ChannelRegistry {
    channels: HashMap<ChannelType, Arc<dyn Channel>>,
    dingtalk: Option<Arc<DingTalkChannel>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_channel(mut self, channel: Arc<dyn Channel>) -> Self {
        self.channels.insert(channel.channel_type(), channel);
        self
    }

    pub fn get(&self, channel_type: ChannelType) -> Option<Arc<dyn Channel>> {
        self.channels.get(&channel_type).cloned()
    }

    pub fn with_dingtalk(mut self, channel: Arc<DingTalkChannel>) -> Self {
        self.channels.insert(
            ChannelType::DingTalk,
            channel.clone() as Arc<dyn Channel>,
        );
        self.dingtalk = Some(channel);
        self
    }

    pub fn dingtalk(&self) -> Option<Arc<DingTalkChannel>> {
        self.dingtalk.clone()
    }

    pub fn has_channel(&self, channel_type: ChannelType) -> bool {
        self.channels.contains_key(&channel_type)
    }

    pub fn capability_flags(&self, channel_type: ChannelType) -> Option<ChannelCapabilityFlags> {
        self.channels
            .get(&channel_type)
            .map(|channel| channel.capability_flags())
    }

    pub fn registered_channel_types(&self) -> Vec<ChannelType> {
        let mut types = self.channels.keys().copied().collect::<Vec<_>>();
        types.sort_by_key(|channel_type| channel_type.to_string());
        types
    }

    pub fn registered_channels(&self) -> Vec<RegisteredChannel> {
        let mut channels = self
            .channels
            .iter()
            .map(|(channel_type, channel)| RegisteredChannel {
                channel_type: *channel_type,
                capability_flags: channel.capability_flags(),
            })
            .collect::<Vec<_>>();
        channels.sort_by_key(|channel| channel.channel_type.to_string());
        channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_has_no_channels() {
        let registry = ChannelRegistry::new();

        assert!(registry.dingtalk().is_none());
        assert!(!registry.has_channel(ChannelType::DingTalk));
        assert_eq!(registry.capability_flags(ChannelType::DingTalk), None);
        assert!(registry.registered_channel_types().is_empty());
        assert!(registry.registered_channels().is_empty());
    }

    #[test]
    fn registry_tracks_dingtalk_channel() {
        let channel = Arc::new(DingTalkChannel::new(
            "app-key".to_string(),
            "app-secret".to_string(),
            1,
            None,
        ));
        let registry = ChannelRegistry::new().with_dingtalk(Arc::clone(&channel));

        assert!(registry.dingtalk().is_some());
        assert!(registry.has_channel(ChannelType::DingTalk));
        assert!(registry.get(ChannelType::DingTalk).is_some());
        assert_eq!(
            registry.capability_flags(ChannelType::DingTalk),
            Some(
                ChannelCapabilityFlags::SEND_TO_RECIPIENT
                    | ChannelCapabilityFlags::INBOUND_WEBHOOK
                    | ChannelCapabilityFlags::REPLY_CONTEXT,
            )
        );
        assert_eq!(registry.registered_channel_types(), vec![ChannelType::DingTalk]);
        assert_eq!(
            registry.registered_channels(),
            vec![RegisteredChannel {
                channel_type: ChannelType::DingTalk,
                capability_flags: ChannelCapabilityFlags::SEND_TO_RECIPIENT
                    | ChannelCapabilityFlags::INBOUND_WEBHOOK
                    | ChannelCapabilityFlags::REPLY_CONTEXT,
            }]
        );
    }
}
