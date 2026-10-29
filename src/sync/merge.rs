//! Three-way conflict classification for sync.

#[derive(Debug, PartialEq, Eq)]
pub enum Decision<T> {
    Merged(Option<T>),
    Conflict { local: Option<T>, remote: Option<T> },
}

pub fn merge<T: Clone + Eq>(
    base: Option<&T>,
    local: Option<&T>,
    remote: Option<&T>,
) -> Decision<T> {
    if local == remote {
        Decision::Merged(local.cloned())
    } else if local == base {
        Decision::Merged(remote.cloned())
    } else if remote == base {
        Decision::Merged(local.cloned())
    } else {
        Decision::Conflict {
            local: local.cloned(),
            remote: remote.cloned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_independent_changes_and_preserves_conflicts() {
        assert_eq!(
            merge(Some(&1), Some(&1), Some(&2)),
            Decision::Merged(Some(2))
        );
        assert_eq!(
            merge(Some(&1), Some(&2), Some(&1)),
            Decision::Merged(Some(2))
        );
        assert_eq!(
            merge(Some(&1), Some(&2), Some(&3)),
            Decision::Conflict {
                local: Some(2),
                remote: Some(3)
            }
        );
        assert_eq!(
            merge(Some(&1), None, Some(&2)),
            Decision::Conflict {
                local: None,
                remote: Some(2)
            }
        );
        assert_eq!(merge(Some(&1), None, Some(&1)), Decision::Merged(None));
    }
}
