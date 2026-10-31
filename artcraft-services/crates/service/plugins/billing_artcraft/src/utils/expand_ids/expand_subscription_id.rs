use stripe_shared::Subscription;
use stripe_types::Expandable;

pub fn expand_subscription_id(expandable_subscription: &Expandable<Subscription>) -> String {
  match expandable_subscription {
    Expandable::Id(id) => id.to_string(),
    Expandable::Object(subscription) => subscription.id.to_string(),
  }
}
