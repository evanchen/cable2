use ::prost::Message;
use protogen;
use protogen::output::allprotos::Item;

fn serialize(item: &mut Item) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(item.encoded_len());
    // Unwrap is safe, since we have reserved sufficient capacity in the vector.
    item.encode(&mut buf).unwrap();
    buf
}

fn deserialize(buf: &[u8]) -> Result<Item, prost::DecodeError> {
    Item::decode(buf)
}

#[test]
fn itemtest() {
    let mut item = Item::default();
    item.uid = 123;
    item.id = 666;
    let buf = serialize(&mut item);
    println!("{:?}", buf);

    let item2 = deserialize(&buf).unwrap();
    println!("{:?}", item2);
}
