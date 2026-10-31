use crate::configs::credits_packs::stripe_artcraft_credits_pack_info::StripeArtcraftCreditsPackInfo;
use crate::configs::subscriptions::stripe_artcraft_subscription_info::StripeArtcraftSubscriptionInfo;

#[derive(Debug, Clone)]
pub enum StripeArtcraftGenericProductInfo {
  CreditsPack(StripeArtcraftCreditsPackInfo),
  Subscription(StripeArtcraftSubscriptionInfo),
}
