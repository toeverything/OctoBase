use super::*;
use nom::multi::count;

#[derive(Debug, PartialEq)]
struct AwarenessClientState {
    clock: u64,
    // content is usually a json
    content: String,
}

fn read_awareness_state(input: &[u8]) -> IResult<&[u8], (u64, AwarenessClientState)> {
    let (tail, client_id) = read_var_u64(input)?;
    let (tail, clock) = read_var_u64(tail)?;
    let (tail, content) = read_var_string(tail)?;

    Ok((tail, (client_id, AwarenessClientState { clock, content })))
}

fn read_awareness(input: &[u8]) -> IResult<&[u8], HashMap<u64, AwarenessClientState>> {
    let (tail, len) = read_var_u64(&input)?;

    let (tail, messages) = count(read_awareness_state, len as usize)(tail)?;

    Ok((tail, messages.into_iter().collect()))
}

fn write_awareness_state<W: Write>(
    buffer: &mut W,
    client_id: u64,
    state: &AwarenessClientState,
) -> Result<(), IoError> {
    write_var_u64(buffer, client_id)?;
    write_var_u64(buffer, state.clock)?;
    write_var_string(buffer, state.content.clone())?;

    Ok(())
}

fn write_awareness<W: Write>(
    buffer: &mut W,
    clients: &HashMap<u64, AwarenessClientState>,
) -> Result<(), IoError> {
    write_var_u64(buffer, clients.len() as u64)?;

    for (client_id, state) in clients {
        println!("{}", client_id);
        write_awareness_state(buffer, *client_id, state)?;
    }

    Ok(())
}

// awareness state message
pub struct AwarenessMessage {
    clients: HashMap<u64, AwarenessClientState>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_awareness() {
        let input = [
            3, // count of state
            1, 5, 1, 1, // first state
            2, 10, 2, 2, 3, // second state
            5, 5, 5, 1, 2, 3, 4, 5, // third state
        ];

        let expected = HashMap::from([
            (
                5,
                AwarenessClientState {
                    clock: 5,
                    content: String::from_utf8(vec![1, 2, 3, 4, 5]).unwrap(),
                },
            ),
            (
                2,
                AwarenessClientState {
                    clock: 10,
                    content: String::from_utf8(vec![2, 3]).unwrap(),
                },
            ),
            (
                1,
                AwarenessClientState {
                    clock: 5,
                    content: String::from_utf8(vec![1]).unwrap(),
                },
            ),
        ]);

        {
            let (tail, result) = read_awareness(&input).unwrap();
            assert!(tail.is_empty());
            assert_eq!(result, expected);
        }

        {
            let mut buffer = Vec::new();
            // hashmap has not a ordered keys, so buffer not equal each write
            // we need re-parse the buffer to check result
            write_awareness(&mut buffer, &expected).unwrap();
            let (tail, result) = read_awareness(&buffer).unwrap();
            assert!(tail.is_empty());
            assert_eq!(result, expected);
        }
    }
}
