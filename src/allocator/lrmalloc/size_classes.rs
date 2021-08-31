/// This is generated using the Series given in the Paper starting with X = 10
/// and is then filtered down to only contain sizes that are divisible by 8 for
/// alignemnt
const SIZE_CLASSES: [usize; 17] = [
    1024, 1280, 1536, 1792, 2048, 2560, 3072, 3584, 4096, 5120, 6144, 7168, 8192, 10240, 12288,
    14336, 16384,
];

pub const fn size_class_count() -> usize {
    SIZE_CLASSES.len()
}

pub fn get_size_class_index(size: usize) -> Option<usize> {
    for (index, class_size) in SIZE_CLASSES.iter().enumerate() {
        if size < *class_size {
            return Some(index);
        }
    }

    None
}

pub fn get_block_size(size_class: usize) -> usize {
    SIZE_CLASSES[size_class]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smallest_size_class() {
        let size = 16;
        let expected = Some(0);

        assert_eq!(expected, get_size_class_index(size));
    }

    #[test]
    fn middle_size_class() {
        let size = 5500;
        let expected = Some(10);

        assert_eq!(expected, get_size_class_index(size));
    }

    #[test]
    fn too_large_size() {
        let size = 20000;
        let expected = None;

        assert_eq!(expected, get_size_class_index(size));
    }
}
