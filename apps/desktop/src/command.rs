//! The command line at the foot of the window: `seed 275`, `size large`, `step +1`, `random`, `reset`, `help`.
//! Parsing is separate from acting on a command so it can be tested without a window.

use tapiti_core::params::Size;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Seed(u32),
    Size(Size),
    /// A number of seeds forward (positive) or back (negative).
    Step(i64),
    Random,
    Reset,
    Help,
    Layer(Layer),
    /// The text of the filter of the tree (empty clears it).
    Filter(String),
    /// A group of the tree to open and scroll to, by id or by part of its title.
    Group(String),
}

/// What the plan can be tinted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    None,
    Height,
    Water,
    Heat,
    Light,
    Events,
}

impl Layer {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Height => "height",
            Self::Water => "water",
            Self::Heat => "heat",
            Self::Light => "light",
            Self::Events => "events",
        }
    }
}

pub const HELP: &str =
    "seed N · size S|M|L|XL · step ±N · random · reset · layer NAME · filter TEXT · group NAME";

pub fn parse(line: &str) -> Result<Command, String> {
    let mut words = line.split_whitespace();
    let Some(name) = words.next() else {
        return Err(String::new());
    };
    let name = name.to_lowercase();
    // The filter and the group take the rest of the line; the other commands take at most one word.
    if name == "filter" || name == "group" {
        let rest = words.collect::<Vec<_>>().join(" ");
        return match (name.as_str(), rest.is_empty()) {
            ("filter", _) => Ok(Command::Filter(rest)),
            (_, false) => Ok(Command::Group(rest)),
            _ => Err("group needs a name, such as corridors".into()),
        };
    }
    let arg = words.next();
    if words.next().is_some() {
        return Err(format!("too many words: {line}"));
    }
    match (name.as_str(), arg) {
        ("seed", Some(n)) => n
            .parse::<u32>()
            .map(Command::Seed)
            .map_err(|_| format!("seed must be a number from 0 to {}: {n}", u32::MAX)),
        ("seed", None) => Err("seed needs a number".into()),
        ("size", Some(s)) => Size::ALL
            .iter()
            .enumerate()
            .find(|(i, size)| {
                let s = s.to_lowercase();
                size.name() == s || ["s", "m", "l", "xl"][*i] == s
            })
            .map(|(_, size)| size)
            .map(|size| Command::Size(*size))
            .ok_or_else(|| format!("size is small, medium, large or huge (s, m, l, xl): {s}")),
        ("size", None) => Err("size needs small, medium, large or huge".into()),
        ("step", arg) => match arg.unwrap_or("+1").trim_start_matches('+').parse::<i64>() {
            Ok(n) => Ok(Command::Step(n)),
            Err(_) => Err(format!(
                "step needs a number such as +1 or -3: {}",
                arg.unwrap_or("")
            )),
        },
        ("layer", Some(l)) => match l.to_lowercase().as_str() {
            "none" | "plain" | "off" => Ok(Command::Layer(Layer::None)),
            "height" => Ok(Command::Layer(Layer::Height)),
            "water" => Ok(Command::Layer(Layer::Water)),
            "heat" | "temperature" => Ok(Command::Layer(Layer::Heat)),
            "light" => Ok(Command::Layer(Layer::Light)),
            "events" => Ok(Command::Layer(Layer::Events)),
            other => Err(format!(
                "layer is none, height, water, heat, light or events: {other}"
            )),
        },
        ("layer", None) => {
            Err("layer needs a name: none, height, water, heat, light or events".into())
        }
        ("random", None) => Ok(Command::Random),
        ("reset", None) => Ok(Command::Reset),
        ("help" | "?", None) => Ok(Command::Help),
        (other, _) => Err(format!("unknown command: {other} (try help)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commands_parse() {
        assert_eq!(parse("seed 275"), Ok(Command::Seed(275)));
        assert_eq!(parse("  SEED   4294967295 "), Ok(Command::Seed(u32::MAX)));
        assert_eq!(parse("size large"), Ok(Command::Size(Size::Large)));
        assert_eq!(parse("step"), Ok(Command::Step(1)));
        assert_eq!(parse("step +3"), Ok(Command::Step(3)));
        assert_eq!(parse("step -2"), Ok(Command::Step(-2)));
        assert_eq!(parse("random"), Ok(Command::Random));
        assert_eq!(parse("reset"), Ok(Command::Reset));
        assert_eq!(parse("?"), Ok(Command::Help));
        assert_eq!(parse("size xl"), Ok(Command::Size(Size::Huge)));
        assert_eq!(parse("layer heat"), Ok(Command::Layer(Layer::Heat)));
        assert_eq!(parse("layer none"), Ok(Command::Layer(Layer::None)));
        assert_eq!(
            parse("filter max bends"),
            Ok(Command::Filter("max bends".into()))
        );
        assert_eq!(parse("filter"), Ok(Command::Filter(String::new())));
        assert_eq!(
            parse("group corridors"),
            Ok(Command::Group("corridors".into()))
        );
    }

    #[test]
    fn mistakes_say_what_is_wrong() {
        assert!(parse("seed").unwrap_err().contains("number"));
        assert!(parse("seed -1").unwrap_err().contains("0 to"));
        assert!(parse("seed 4294967296").is_err());
        assert!(parse("size tiny").unwrap_err().contains("small"));
        assert!(parse("step x").unwrap_err().contains("number"));
        assert!(parse("seed 1 2").unwrap_err().contains("too many"));
        assert!(parse("jump").unwrap_err().contains("unknown"));
        assert!(parse("layer lava").unwrap_err().contains("none, height"));
        assert!(parse("layer").is_err());
        assert!(parse("group").unwrap_err().contains("name"));
        assert_eq!(parse("   "), Err(String::new()));
    }
}
