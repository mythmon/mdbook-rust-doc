//! All sorts of crustaceans.

/// A crab.
pub struct Crab {
    /// The number of legs this crab has. Probably 8, but there are some weird
    /// crabs out there!
    pub num_legs: u8,
}

/// Some people eat crabs
pub struct CookedCrab(
    /// The crab that was cooked.
    Crab,
    /// A description of how it was cooked.
    String,
);

/// Lobster colors, according to Wikipedia.
pub enum LobsterColor {
    /// Also called white; translucent; ghost; crystal.
    Albino,
    /// Also called pastel. Possibly a sub-type of albino
    CottonCandy,
    /// Caused by a genetic defect.
    Blue,
    /// Color of *Eve*, a lobster found in Maryland in 2019.
    Calico,
    /// It's just orange
    Orange,
    /// Almost all split-coloreds are hermaphroditic.
    SplitColored {
        /// The color that is more prevalent on the lobster.
        primary: Box<LobsterColor>,
        /// The color that is less prevalent on the lobster.
        secondary: Box<LobsterColor>,
    },
    /// The typical lobster.
    Red(
        /// A description of the intensity of the red
        String,
    ),
    /// Like a lemon.
    Yellow,
    /// Half of a Halloween lobster.
    Black,
}

impl LobsterColor {
    /// A common split colored lobster.
    pub fn halloween() -> Self {
        Self::SplitColored {
            primary: Box::new(Self::Orange),
            secondary: Box::new(Self::Black),
        }
    }
}
