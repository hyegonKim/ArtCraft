
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PlanCategory {
    /// Free plans cost nothing and are the lowest tiers of service.
    Free,
    /// Certain users will get premium features for free by contributing to FakeYou.
    LoyaltyReward,
    /// Paid plans come with various variable premium perks according to plan level.
    Paid,
}

