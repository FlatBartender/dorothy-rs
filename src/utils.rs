pub struct FoldStrlenState {
    strings: Vec<Vec<String>>,
    partition_size: usize,
    current_character_count: usize,
    total_character_count: usize,
}

impl FoldStrlenState {
    pub fn new(nb_chars: usize) -> Self {
        FoldStrlenState {
            strings: Vec::new(),
            partition_size: nb_chars,
            current_character_count: 0,
            total_character_count: 0,
        }
    }

    pub fn extract(self) -> Vec<Vec<String>> {
        self.strings
    }
}

pub type FoldStrlenStateResult = Result<FoldStrlenState, String>;

pub fn fold_by_strlen(mut state: FoldStrlenState, item: String) -> FoldStrlenStateResult {
    if item.len() > state.partition_size {
        return Err("A string is too long.".to_string());
    }

    let mut vector = if state.current_character_count + item.len() > state.partition_size {
        // Need to allocate a new vector to store the next lines.
        state.current_character_count = 0;
        Vec::new()
    } else {
        // Get the last vector, or an empty one if no work has been done prior.
        // Unwrapping here should be safe because all the paths give a Vec.
        state.strings.pop().or(Some(Vec::new())).unwrap()
    };

    state.current_character_count += item.len();
    state.total_character_count += item.len();
    vector.push(item);

    state.strings.push(vector);

    Ok(state)
}
