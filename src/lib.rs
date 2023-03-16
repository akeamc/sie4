use item::Item;
use nom::{multi::many0, IResult};
use parsers::take_till_label;

pub mod item;
mod parsers;

pub fn parse_items(i: &str) -> IResult<&str, Vec<Item>> {
    many0(|i| {
        let (i, _) = take_till_label(i)?;
        Item::parse(i)
    })(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_items_strange_input() {
        assert_eq!(parse_items(""), Ok(("", vec![])));
    }
}
