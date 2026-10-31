import { BillingApi } from "@storyteller/api";

// True when the signed-in user has at least one active subscription. Drives
// post-auth routing: users without one are sent to /pricing. Fails closed
// (returns false) so a billing error still routes toward pricing rather than
// silently skipping it.
export async function hasActiveSubscription(): Promise<boolean> {
  try {
    const response = await new BillingApi().ListActiveSubscriptions();
    return (
      response.success && (response.data?.active_subscriptions.length ?? 0) > 0
    );
  } catch {
    return false;
  }
}
