pub struct OwnedBytes<T> {
    data: Vec<u8>,
    owner: Option<T>,
}

impl<T> OwnedBytes<T> {
    pub fn new(data: Vec<u8>, owner: T) -> Self {
        Self {
            data,
            owner: Some(owner),
        }
    }

    pub fn into_vec(mut self) -> Vec<u8> {
        let data = std::mem::take(&mut self.data);
        std::mem::forget(self);
        data
    }

    pub fn has_owner(&self) -> bool {
        self.owner.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_preserves_bytes() {
        let bytes = OwnedBytes::new(vec![1, 2, 3], ());
        assert_eq!(bytes.into_vec(), vec![1, 2, 3]);
    }
}
