//! The curated builder labels: who a builder address is, read by a person. Ported from
//! stats.outcome.xyz's `builder-meta.ts` by way of hip4-tui (`tui/src/builders.rs`, where it
//! arrived on 2026-09-18) and kept as data, with that provenance: a label is someone's reading of
//! an address, nothing the venue or a provider says.

use hl_core::Address;

/// Where the list comes from.
pub const SOURCE: &str = "stats.outcome.xyz builder-meta.ts, by way of hip4-tui tui/src/builders.rs";

/// When hip4-tui took it.
pub const AS_OF: &str = "2026-09-18";

/// What sort of builder an address is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A product people use.
    Real,
    Tool,
    Bot,
    /// One person's own address.
    Personal,
    /// A perp venue.
    Perp,
    Premium,
    /// Listed, with nobody's reading of it.
    Unknown,
}

impl Kind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Kind::Real => "product",
            Kind::Tool => "tool",
            Kind::Bot => "bot",
            Kind::Personal => "personal",
            Kind::Perp => "perp venue",
            Kind::Premium => "premium",
            Kind::Unknown => "",
        }
    }
}

/// One curated address. An empty name is an address classified but not named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label {
    /// Lowercase, as the list keeps it.
    pub address: &'static str,
    pub name: &'static str,
    pub kind: Kind,
}

/// The list, in its source's order.
pub const LABELS: [Label; 59] = [
    Label { address: "0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704", name: "OUTCOME", kind: Kind::Real },
    Label { address: "0x6967d7acd1ec5f210e9590666498e0cc5d13a843", name: "DFU", kind: Kind::Premium },
    Label { address: "0xcab561b82f58ca7104105f52e5563a83a948252c", name: "HYPERBEAT", kind: Kind::Bot },
    Label { address: "0xb84c7fb41ee7d8781e2b0d59eed2accd2ae99533", name: "MOONBOT", kind: Kind::Tool },
    Label { address: "0x0fdd59414989f52989973fef8995b7d10248cc18", name: "LIQUIDICTION", kind: Kind::Tool },
    Label { address: "0x1c64d7e3386e005e03ea7a5e669eee3f63183a47", name: "", kind: Kind::Personal },
    Label { address: "0xf5727c8d620418a057777282d92e1c6b12610f86", name: "", kind: Kind::Personal },
    Label { address: "0x56e48ec7bd4b07312707f5e3dbfe2baf85137107", name: "", kind: Kind::Personal },
    Label { address: "0xb84168cf3be63c6b8dad05ff5d755e97432ff80b", name: "PHANTOM", kind: Kind::Perp },
    Label { address: "0xd4e181a5d7f84e5b3f6d60704a38b87c6d433f8f", name: "", kind: Kind::Personal },
    Label { address: "0x518d89b89ea2731973231df4de99497a45862dde", name: "HYPERSWAPX", kind: Kind::Perp },
    Label { address: "0x0c3f9fa51ef98213c033dded8f7351aee2fdbe34", name: "", kind: Kind::Premium },
    Label { address: "0x4100ffccff56d8d0d3161ab13187936c688aff27", name: "HYPERVIEW", kind: Kind::Personal },
    Label { address: "0x74c362cd3a141769f38c48d66ee51b1938ea4bd0", name: "", kind: Kind::Personal },
    Label { address: "0xd58ee5dc9cb03df60843c6234644aa535b08a8b9", name: "", kind: Kind::Perp },
    Label { address: "0x659e0c2473285d38281181231aba1154ee535955", name: "", kind: Kind::Personal },
    Label { address: "0x8608d0329889014c7d62182a48327dfec55cf439", name: "", kind: Kind::Personal },
    Label { address: "0x6d10ea66310f8b9a5544f392fb6347151f01c01f", name: "", kind: Kind::Personal },
    Label { address: "0xcf7e559ada0e979bbbff1b263ea60185a9a594e4", name: "", kind: Kind::Personal },
    Label { address: "0x5a3bc60b0a99a7f4fbf0d15554fa5fe88e7628c2", name: "MINARA AI", kind: Kind::Real },
    Label { address: "0x1924b8561eef20e70ede628a296175d358be80e5", name: "BASEDAPP", kind: Kind::Real },
    Label { address: "0x92345453ce2000642d7d4ceeae4fccc6c2e41d23", name: "TRADOOR", kind: Kind::Tool },
    Label { address: "0x008adf65b8c404e8bba73f18671306066643761f", name: "CRO.TRADE", kind: Kind::Tool },
    Label { address: "0x2ae8429e13217f9452d50fc4f01c09913dc16cfe", name: "", kind: Kind::Tool },
    Label { address: "0x60e26ec284d4cd118547611f109f84207fa4bb14", name: "", kind: Kind::Tool },
    Label { address: "0x999a4b5f268a8fbf33736feff360d462ad248dbf", name: "TREAD.FI", kind: Kind::Personal },
    Label { address: "0x6fd5f1b94aa4963a8220b931c14365bc4bdbadcc", name: "", kind: Kind::Personal },
    Label { address: "0x4950994884602d1b6c6d96e4fe30f58205c39395", name: "DREAMCASH", kind: Kind::Personal },
    Label { address: "0x12399362b16042e356ebdb4b1af0477046c68ec3", name: "", kind: Kind::Personal },
    Label { address: "0xbb8392ca7c533e3bfde1108f3a08adb5868613c0", name: "", kind: Kind::Personal },
    Label { address: "0xadbedb0e36af90bc5a9893c07c34b15ce860f3f7", name: "", kind: Kind::Personal },
    Label { address: "0x22047776933bc123d0602ed17aaf0d2f5647df0c", name: "", kind: Kind::Personal },
    Label { address: "0x450c868e83a5bf6907d67c79450f149ada9d176b", name: "HYDE", kind: Kind::Bot },
    Label { address: "0x6d422a488f3f44b4b28506b27020c1d54cb1b711", name: "", kind: Kind::Bot },
    Label { address: "0x640aa211b4298f18927d8e856d1d8c37d22a00bf", name: "", kind: Kind::Personal },
    Label { address: "0x0fc9c144bda82b579f6085621b89f5dc6cfeed4c", name: "", kind: Kind::Personal },
    Label { address: "0x4752f5d4ea2bd35ec257228074053f19bdb41e8c", name: "", kind: Kind::Personal },
    Label { address: "0x733f40a4fa0cd13d59abade04b9ed2e9acac6457", name: "", kind: Kind::Personal },
    Label { address: "0xd01a433d0003c949025463f550018bc6ed0ea7aa", name: "", kind: Kind::Personal },
    Label { address: "0x19d5bd0d8e29bfda20e6d7886a025a89b1758fe1", name: "", kind: Kind::Unknown },
    Label { address: "0x6eb8b5210603c9959331bc5b7460eb7301fa636c", name: "", kind: Kind::Real },
    Label { address: "0x36be02a397e969e010ccbd7333f4169f66b8989f", name: "", kind: Kind::Unknown },
    Label { address: "0xd6ffefb241c04ace1e77fea508e5beb59492a3cb", name: "", kind: Kind::Unknown },
    Label { address: "0x9f6e2ee01f4f1958f4e14b8e8e50bc0131cb7e36", name: "", kind: Kind::Personal },
    Label { address: "0x113f059d7a863fbca1ff60df5b1954dbec9f91dd", name: "", kind: Kind::Unknown },
    Label { address: "0x71b09a08257078a4d642f7dd7315e656c837329a", name: "", kind: Kind::Personal },
    Label { address: "0xaa1d89f333857ed78f8434cc4f896a9293efe65c", name: "", kind: Kind::Personal },
    Label { address: "0x8da41fa8079a68fb96c1331830821f1ff43db66e", name: "", kind: Kind::Personal },
    Label { address: "0x8d62d3000ef0639d1fc9667d06be7bb98d9993f5", name: "", kind: Kind::Unknown },
    Label { address: "0x338c482c9b89047b1caf9f8ebb504aa0e8550694", name: "", kind: Kind::Unknown },
    Label { address: "0x0c8d970c462726e014ad36f6c5a63e99db48a8e7", name: "", kind: Kind::Personal },
    Label { address: "0xee41f05496bc30dbd29c96bc31283c7e9f062192", name: "", kind: Kind::Unknown },
    Label { address: "0x2e10360cbfb68080b72c17f35633700e75fe461b", name: "", kind: Kind::Personal },
    Label { address: "0x558c5035d583a6a12597f0bf2f91fba556069d4a", name: "", kind: Kind::Personal },
    Label { address: "0x32f6940795d6d484a6d29b6b628ebaff80a0c779", name: "", kind: Kind::Personal },
    Label { address: "0xce66462239c2307532c11d4195083f4645606f0e", name: "", kind: Kind::Personal },
    Label { address: "0xc742cf48524b07173dad1642c09c1dff504eb71a", name: "", kind: Kind::Personal },
    Label { address: "0x22647020fa62d6f7881c679007df63f759b09acb", name: "", kind: Kind::Personal },
    Label { address: "0x5ef4deeb76f87d979d0ddc8c51f5b4f65d1c972a", name: "", kind: Kind::Personal },
];

/// Every address the list names.
pub fn known() -> impl Iterator<Item = Address> {
    LABELS.iter().filter_map(|one| one.address.parse().ok())
}

/// A builder's curated name, where it has one.
#[must_use]
pub fn label(builder: Address) -> Option<&'static str> {
    found(builder).map(|one| one.name).filter(|name| !name.is_empty())
}

/// A builder's curated kind, where anybody classified it.
#[must_use]
pub fn kind(builder: Address) -> Option<Kind> {
    found(builder).map(|one| one.kind).filter(|kind| *kind != Kind::Unknown)
}

fn found(builder: Address) -> Option<&'static Label> {
    let hex = builder.to_hex();
    LABELS.iter().find(|one| one.address == hex)
}

#[cfg(test)]
#[path = "../tests/labels.rs"]
mod tests;
