//! JSONPath implementation for serde_json::Value manipulation
//!
//! Public module for manipulating JSON data using path expressions.
//!
//! Supports paths like:
//! - `/`: Root
//! - `/attr`: Object attribute
//! - `/attr[0]`: Array indexing
//! - `/attr/attr2[-1]`: Nested access with negative indexing
//!
//! Provides operations:
//! - get: Retrieve a value
//! - get_mut: Get a mutable reference
//! - remove: Remove an element
//! - set: Set a value
//! - insert: Insert a value

use serde_json::Value;
use std::fmt;

/// Error type for JSONPath operations
#[derive(Debug)]
pub enum JsonPathError {
    /// Path syntax is invalid
    InvalidPath(String),
    /// A segment in the path doesn't exist
    PathNotFound(String),
    /// Expected object but found different type
    NotAnObject(String),
    /// Expected array but found different type
    NotAnArray(String),
    /// Index is out of bounds
    IndexOutOfBounds(String),
    /// General operation failure
    OperationFailed(String),
}

impl fmt::Display for JsonPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            Self::PathNotFound(msg) => write!(f, "Path not found: {}", msg),
            Self::NotAnObject(msg) => write!(f, "Not an object: {}", msg),
            Self::NotAnArray(msg) => write!(f, "Not an array: {}", msg),
            Self::IndexOutOfBounds(msg) => write!(f, "Index out of bounds: {}", msg),
            Self::OperationFailed(msg) => write!(f, "Operation failed: {}", msg),
        }
    }
}

impl std::error::Error for JsonPathError {}

/// A segment in a JSONPath
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment<'a> {
    /// Root segment (/)
    Root,
    /// Object attribute (e.g., "users")
    Property(&'a str),
    /// Array index (e.g., [0], [-1])
    Index(isize),
    /// Range of indices (e.g., [0..10], [0..], [..5], [..])
    Range(Option<isize>, Option<isize>),
}

/// Parse a JSONPath string into segments
pub fn parse_path(path: &str) -> Result<Vec<PathSegment<'_>>, JsonPathError> {
    if path.is_empty() {
        return Err(JsonPathError::InvalidPath("Empty path".to_string()));
    }

    let mut segments = Vec::new();

    // Handle root path
    if path == "/" {
        segments.push(PathSegment::Root);
        return Ok(segments);
    }

    // Validate that path starts with "/"
    if !path.starts_with('/') {
        return Err(JsonPathError::InvalidPath(format!(
            "Path must start with '/', got: {}",
            path
        )));
    }

    segments.push(PathSegment::Root);

    // Split the path into segments
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Ok(segments);
    }

    let parts: Vec<&str> = path.split('/').collect();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        // Check if the segment contains an array index
        if part.contains('[') && part.contains(']') {
            let bracket_pos = part.find('[').unwrap();
            let property = &part[0..bracket_pos];
            let index_part = &part[bracket_pos..];

            // Add property segment if it's not empty
            if !property.is_empty() {
                segments.push(PathSegment::Property(property));
            }

            // Parse array index segments
            let indexes: Vec<&str> = index_part
                .trim_matches(|c| c == '[' || c == ']')
                .split("][")
                .collect();

            for idx_str in indexes {
                // Check if it's a range expression (contains '..')
                if idx_str.contains("..") {
                    let range_parts: Vec<&str> = idx_str.split("..").collect();

                    if range_parts.len() != 2 {
                        return Err(JsonPathError::InvalidPath(format!(
                            "Invalid range syntax: {}",
                            idx_str
                        )));
                    }

                    let start = if range_parts[0].is_empty() {
                        None
                    } else {
                        match range_parts[0].parse::<isize>() {
                            Ok(idx) => Some(idx),
                            Err(_) => {
                                return Err(JsonPathError::InvalidPath(format!(
                                    "Invalid range start index: {}",
                                    range_parts[0]
                                )))
                            }
                        }
                    };

                    let end = if range_parts[1].is_empty() {
                        None
                    } else {
                        match range_parts[1].parse::<isize>() {
                            Ok(idx) => Some(idx),
                            Err(_) => {
                                return Err(JsonPathError::InvalidPath(format!(
                                    "Invalid range end index: {}",
                                    range_parts[1]
                                )))
                            }
                        }
                    };

                    segments.push(PathSegment::Range(start, end));
                } else if idx_str.is_empty() {
                    // Empty index [] should be rejected
                    return Err(JsonPathError::InvalidPath(
                        "Empty array index [] is not valid".to_string(),
                    ));
                } else {
                    // Regular index
                    match idx_str.parse::<isize>() {
                        Ok(idx) => segments.push(PathSegment::Index(idx)),
                        Err(_) => {
                            return Err(JsonPathError::InvalidPath(format!(
                                "Invalid array index: {}",
                                idx_str
                            )))
                        }
                    }
                }
            }
        } else {
            // Simple property segment
            segments.push(PathSegment::Property(part));
        }
    }

    Ok(segments)
}

/// Convert a negative index to a positive one, based on array length
fn normalize_index(index: isize, array_len: usize) -> Result<usize, JsonPathError> {
    if index >= 0 && index < array_len as isize {
        Ok(index as usize)
    } else if index < 0 && -index <= array_len as isize {
        Ok((array_len as isize + index) as usize)
    } else {
        Err(JsonPathError::IndexOutOfBounds(format!(
            "Index {} out of bounds for array of length {}",
            index, array_len
        )))
    }
}

/// Traverse JSON using path segments and return a reference to the value
pub fn traverse<'a, 'b>(
    json: &'a Value,
    segments: &'b [PathSegment<'b>],
) -> Result<&'a Value, JsonPathError> {
    if segments.is_empty() {
        return Err(JsonPathError::InvalidPath(
            "Empty path segments".to_string(),
        ));
    }

    // Start with root
    let mut current = json;

    // Skip root segment in iteration (if it exists)
    let start_idx = if matches!(segments[0], PathSegment::Root) {
        1
    } else {
        0
    };

    for segment in &segments[start_idx..] {
        match segment {
            PathSegment::Root => unreachable!(), // Root should only be at the start

            PathSegment::Property(key) => {
                if let Value::Object(obj) = current {
                    if let Some(value) = obj.get(*key) {
                        current = value;
                    } else {
                        return Err(JsonPathError::PathNotFound(format!(
                            "Property '{}' not found",
                            key
                        )));
                    }
                } else {
                    return Err(JsonPathError::NotAnObject(format!(
                        "Expected object but found: {:?}",
                        current
                    )));
                }
            }

            PathSegment::Index(idx) => {
                if let Value::Array(arr) = current {
                    let normalized_idx = normalize_index(*idx, arr.len())?;
                    current = &arr[normalized_idx];
                } else {
                    return Err(JsonPathError::NotAnArray(format!(
                        "Expected array but found: {:?}",
                        current
                    )));
                }
            }

            PathSegment::Range(_, _) => {
                return Err(JsonPathError::InvalidPath(
                    "Range operator cannot be used with traverse".to_string(),
                ));
            }
        }
    }

    Ok(current)
}

/// Traverse JSON using path segments and return a mutable reference to the value
pub fn traverse_mut<'a, 'b>(
    json: &'a mut Value,
    segments: &'b [PathSegment<'b>],
) -> Result<&'a mut Value, JsonPathError> {
    if segments.is_empty() {
        return Err(JsonPathError::InvalidPath(
            "Empty path segments".to_string(),
        ));
    }

    // Start with root
    let mut current = json;

    // Skip root segment in iteration (if it exists)
    let start_idx = if matches!(segments[0], PathSegment::Root) {
        1
    } else {
        0
    };

    for segment in &segments[start_idx..] {
        match segment {
            PathSegment::Root => unreachable!(), // Root should only be at the start

            PathSegment::Property(key) => {
                if let Value::Object(obj) = current {
                    if let Some(value) = obj.get_mut(*key) {
                        current = value;
                    } else {
                        return Err(JsonPathError::PathNotFound(format!(
                            "Property '{}' not found",
                            key
                        )));
                    }
                } else {
                    return Err(JsonPathError::NotAnObject(format!(
                        "Expected object but found: {:?}",
                        current
                    )));
                }
            }

            PathSegment::Index(idx) => {
                if let Value::Array(arr) = current {
                    let normalized_idx = normalize_index(*idx, arr.len())?;
                    current = &mut arr[normalized_idx];
                } else {
                    return Err(JsonPathError::NotAnArray(format!(
                        "Expected array but found: {:?}",
                        current
                    )));
                }
            }

            PathSegment::Range(_, _) => {
                // Handle specially - this shouldn't be hit normally but is needed when traversing
                // through a range during operations
                return Err(JsonPathError::NotAnArray(format!(
                    "Expected array index but found range: {:?}",
                    segment
                )));
            }
        }
    }

    Ok(current)
}

/// Finds the range segment in path segments and returns (parent segments, range segment index, remaining segments)
fn find_range_segment<'a>(
    segments: &'a [PathSegment<'a>],
) -> Result<(Option<&'a [PathSegment<'a>]>, usize, &'a [PathSegment<'a>]), JsonPathError> {
    for (i, segment) in segments.iter().enumerate() {
        if let PathSegment::Range(_, _) = segment {
            // Return segments before range, range index, and segments after range
            let parent = if i > 0 { Some(&segments[0..i]) } else { None };
            let remaining = if i < segments.len() - 1 {
                &segments[i + 1..]
            } else {
                &[]
            };
            return Ok((parent, i, remaining));
        }
    }

    Err(JsonPathError::InvalidPath(
        "No range segment found in path".to_string(),
    ))
}

/// Get a reference to a JSON value at the specified path
pub fn get<'a>(json: &'a Value, path: &str) -> Result<&'a Value, JsonPathError> {
    let segments = parse_path(path)?;
    traverse(json, &segments)
}

/// Get a mutable reference to a JSON value at the specified path
pub fn get_mut<'a>(json: &'a mut Value, path: &str) -> Result<&'a mut Value, JsonPathError> {
    let segments = parse_path(path)?;
    traverse_mut(json, &segments)
}

/// Get parent segments (all except the last one)
fn get_parent_segments<'a>(
    segments: &'a [PathSegment<'a>],
) -> Result<&'a [PathSegment<'a>], JsonPathError> {
    if segments.len() < 2 {
        return Err(JsonPathError::InvalidPath(
            "Path too short to have parent".to_string(),
        ));
    }

    Ok(&segments[0..segments.len() - 1])
}

/// Process array indices in a range and apply an operation to each element
fn process_range(
    array: &mut Vec<Value>,
    range: &PathSegment,
    remaining_segments: &[PathSegment],
    f: &mut dyn FnMut(&mut Value, &[PathSegment]) -> Result<(), JsonPathError>,
) -> Result<(), JsonPathError> {
    if let PathSegment::Range(start_opt, end_opt) = range {
        let start = start_opt.unwrap_or(0);
        let end = end_opt.unwrap_or(array.len() as isize);

        // Normalize indices
        let start_idx = normalize_index(start, array.len())?;
        let end_idx = if end < 0 {
            normalize_index(end, array.len())?
        } else {
            std::cmp::min(end as usize, array.len())
        };

        // Apply function to each element in range
        for i in start_idx..end_idx {
            if i < array.len() {
                f(&mut array[i], remaining_segments)?;
            }
        }

        Ok(())
    } else {
        Err(JsonPathError::InvalidPath(
            "Expected range segment".to_string(),
        ))
    }
}

/// Set a value at the specified path
pub fn set(json: &mut Value, path: &str, new_value: Value) -> Result<(), JsonPathError> {
    let segments = parse_path(path)?;

    // Special case for setting root
    if segments.len() == 1 && matches!(segments[0], PathSegment::Root) {
        *json = new_value;
        return Ok(());
    }

    // Check if there's a range segment
    match find_range_segment(&segments) {
        Ok((parent_segments_opt, range_idx, remaining_segments)) => {
            // Function to set value at target path
            let mut set_value = |target: &mut Value, path_segments: &[PathSegment]| {
                if path_segments.is_empty() {
                    // No remaining path, set directly
                    *target = new_value.clone();
                    Ok(())
                } else {
                    // More path to traverse
                    let target_ref = traverse_mut(target, path_segments)?;
                    *target_ref = new_value.clone();
                    Ok(())
                }
            };

            // Get parent of range
            let parent_segments = parent_segments_opt.unwrap_or(&[]);
            let parent = if parent_segments.is_empty() {
                json
            } else {
                traverse_mut(json, parent_segments)?
            };

            // Process the range
            if let Value::Array(arr) = parent {
                process_range(
                    arr,
                    &segments[range_idx],
                    remaining_segments,
                    &mut set_value,
                )
            } else {
                Err(JsonPathError::NotAnArray(format!(
                    "Expected array for range operation but found: {:?}",
                    parent
                )))
            }
        }
        Err(_) => {
            // No range segment - normal set operation
            let parent_segments = get_parent_segments(&segments)?;
            let last_segment = segments.last().unwrap();

            let parent = traverse_mut(json, parent_segments)?;

            match last_segment {
                PathSegment::Root => unreachable!(),

                PathSegment::Property(key) => {
                    if let Value::Object(obj) = parent {
                        obj.insert(key.to_string(), new_value);
                        Ok(())
                    } else {
                        Err(JsonPathError::NotAnObject(format!(
                            "Parent is not an object: {:?}",
                            parent
                        )))
                    }
                }

                PathSegment::Index(idx) => {
                    if let Value::Array(arr) = parent {
                        let normalized_idx = normalize_index(*idx, arr.len())?;
                        arr[normalized_idx] = new_value;
                        Ok(())
                    } else {
                        Err(JsonPathError::NotAnArray(format!(
                            "Parent is not an array: {:?}",
                            parent
                        )))
                    }
                }

                PathSegment::Range(_, _) => unreachable!(),
            }
        }
    }
}

/// Insert a value at the specified path
/// For objects, this acts like set
/// For arrays, this inserts at the specified index
pub fn insert(json: &mut Value, path: &str, new_value: Value) -> Result<(), JsonPathError> {
    let segments = parse_path(path)?;

    // Special case for setting root
    if segments.len() == 1 && matches!(segments[0], PathSegment::Root) {
        return Err(JsonPathError::InvalidPath(
            "Cannot insert at root".to_string(),
        ));
    }

    // Check if there's a range segment
    match find_range_segment(&segments) {
        Ok((parent_segments_opt, range_idx, remaining_segments)) => {
            // Get parent of range
            let parent_segments = parent_segments_opt.unwrap_or(&[]);
            let parent = if parent_segments.is_empty() {
                json
            } else {
                traverse_mut(json, parent_segments)?
            };

            // Function to insert value at target path
            let mut insert_value = |target: &mut Value, path_segments: &[PathSegment]| {
                if path_segments.is_empty() {
                    // No remaining path, set directly
                    *target = new_value.clone();
                    return Ok(());
                }

                if path_segments.len() == 1 {
                    // Last segment - handle specially for insert
                    match path_segments[0] {
                        PathSegment::Property(key) => {
                            if let Value::Object(obj) = target {
                                obj.insert(key.to_string(), new_value.clone());
                                Ok(())
                            } else {
                                Err(JsonPathError::NotAnObject(format!(
                                    "Expected object but found: {:?}",
                                    target
                                )))
                            }
                        }
                        PathSegment::Index(idx) => {
                            if let Value::Array(arr) = target {
                                let normalized_idx = if idx < 0 {
                                    normalize_index(idx, arr.len())?
                                } else {
                                    // Allow insertion at end of array
                                    std::cmp::min(idx as usize, arr.len())
                                };
                                arr.insert(normalized_idx, new_value.clone());
                                Ok(())
                            } else {
                                Err(JsonPathError::NotAnArray(format!(
                                    "Expected array but found: {:?}",
                                    target
                                )))
                            }
                        }
                        _ => Err(JsonPathError::InvalidPath(format!(
                            "Invalid last segment for insert operation: {:?}",
                            path_segments[0]
                        ))),
                    }
                } else {
                    // Multiple segments remaining - traverse to the parent of the last segment
                    let parent_segments = &path_segments[0..path_segments.len() - 1];
                    let last_segment = &path_segments[path_segments.len() - 1];

                    let parent = traverse_mut(target, parent_segments)?;

                    match last_segment {
                        PathSegment::Property(key) => {
                            if let Value::Object(obj) = parent {
                                obj.insert(key.to_string(), new_value.clone());
                                Ok(())
                            } else {
                                Err(JsonPathError::NotAnObject(format!(
                                    "Expected object but found: {:?}",
                                    parent
                                )))
                            }
                        }
                        PathSegment::Index(idx) => {
                            if let Value::Array(arr) = parent {
                                let normalized_idx = if *idx < 0 {
                                    normalize_index(*idx, arr.len())?
                                } else {
                                    // Allow insertion at end of array
                                    std::cmp::min(*idx as usize, arr.len())
                                };
                                arr.insert(normalized_idx, new_value.clone());
                                Ok(())
                            } else {
                                Err(JsonPathError::NotAnArray(format!(
                                    "Expected array but found: {:?}",
                                    parent
                                )))
                            }
                        }
                        _ => Err(JsonPathError::InvalidPath(format!(
                            "Invalid last segment for insert operation: {:?}",
                            last_segment
                        ))),
                    }
                }
            };

            // Process the range
            if let Value::Array(arr) = parent {
                process_range(
                    arr,
                    &segments[range_idx],
                    remaining_segments,
                    &mut insert_value,
                )
            } else {
                Err(JsonPathError::NotAnArray(format!(
                    "Expected array for range operation but found: {:?}",
                    parent
                )))
            }
        }
        Err(_) => {
            // No range segment - normal insert operation
            let parent_segments = get_parent_segments(&segments)?;
            let last_segment = segments.last().unwrap();

            let parent = traverse_mut(json, parent_segments)?;

            match last_segment {
                PathSegment::Root => unreachable!(),

                PathSegment::Property(key) => {
                    if let Value::Object(obj) = parent {
                        obj.insert(key.to_string(), new_value);
                        Ok(())
                    } else {
                        Err(JsonPathError::NotAnObject(format!(
                            "Parent is not an object: {:?}",
                            parent
                        )))
                    }
                }

                PathSegment::Index(idx) => {
                    if let Value::Array(arr) = parent {
                        let normalized_idx = if *idx < 0 {
                            normalize_index(*idx, arr.len())?
                        } else {
                            // Allow insertion at end of array (equivalent to push)
                            std::cmp::min(*idx as usize, arr.len())
                        };

                        arr.insert(normalized_idx, new_value);
                        Ok(())
                    } else {
                        Err(JsonPathError::NotAnArray(format!(
                            "Parent is not an array: {:?}",
                            parent
                        )))
                    }
                }

                PathSegment::Range(_, _) => unreachable!(),
            }
        }
    }
}

/// Remove a value at the specified path and return the removed value
pub fn remove(json: &mut Value, path: &str) -> Result<Value, JsonPathError> {
    let segments = parse_path(path)?;

    // Cannot remove root
    if segments.len() == 1 && matches!(segments[0], PathSegment::Root) {
        return Err(JsonPathError::InvalidPath("Cannot remove root".to_string()));
    }

    // Check if there's a range segment in the path
    match find_range_segment(&segments) {
        Ok((parent_segments_opt, range_idx, remaining_segments)) => {
            // If range is the last segment, use existing logic
            if range_idx == segments.len() - 1 {
                // Get the parent segments
                let parent_segments = get_parent_segments(&segments)?;

                // Get mutable reference to parent
                let parent = traverse_mut(json, parent_segments)?;

                if let Value::Array(arr) = parent {
                    let range_segment = segments.last().unwrap();
                    if let PathSegment::Range(start_opt, end_opt) = range_segment {
                        let start = start_opt.unwrap_or(0);
                        let end = end_opt.unwrap_or(arr.len() as isize);

                        // Normalize indices
                        let start_idx = normalize_index(start, arr.len())?;
                        let end_idx = if end < 0 {
                            normalize_index(end, arr.len())?
                        } else {
                            std::cmp::min(end as usize, arr.len())
                        };

                        // Verify range is valid
                        if start_idx > end_idx {
                            return Err(JsonPathError::InvalidPath(format!(
                                "Invalid range: start index {} is greater than end index {}",
                                start_idx, end_idx
                            )));
                        }

                        // Remove elements in reverse order to avoid shifting issues
                        let mut removed_values = Vec::new();
                        for i in (start_idx..end_idx).rev() {
                            if i < arr.len() {
                                let removed = arr.remove(i);
                                removed_values.push(removed);
                            }
                        }

                        // Reverse the removed values to return them in the original order
                        removed_values.reverse();

                        // Return the removed values as a JSON array
                        Ok(Value::Array(removed_values))
                    } else {
                        unreachable!()
                    }
                } else {
                    Err(JsonPathError::NotAnArray(format!(
                        "Expected array but found: {:?}",
                        parent
                    )))
                }
            } else {
                // Handle range in middle of path - need to collect removed values
                let mut removed_values = Vec::new();

                // Get parent of range
                let parent_segments = parent_segments_opt.unwrap_or(&[]);
                let parent = if parent_segments.is_empty() {
                    json
                } else {
                    traverse_mut(json, parent_segments)?
                };

                // Process the range and collect removed values
                if let Value::Array(arr) = parent {
                    let range_segment = &segments[range_idx];
                    if let PathSegment::Range(start_opt, end_opt) = range_segment {
                        let start = start_opt.unwrap_or(0);
                        let end = end_opt.unwrap_or(arr.len() as isize);

                        // Normalize indices
                        let start_idx = normalize_index(start, arr.len())?;
                        let end_idx = if end < 0 {
                            normalize_index(end, arr.len())?
                        } else {
                            std::cmp::min(end as usize, arr.len())
                        };

                        // Function to remove value from a target using remaining path segments
                        let remove_from_target =
                            |target: &mut Value,
                             segments: &[PathSegment]|
                             -> Result<Value, JsonPathError> {
                                // If there's only one segment left (e.g., "info" in "/messages[..]/info"),
                                // we treat the target as the parent and the segment as the property to remove
                                if segments.len() == 1 {
                                    let last_segment = segments.last().unwrap();

                                    match last_segment {
                                        PathSegment::Property(key) => {
                                            if let Value::Object(obj) = target {
                                                if let Some(removed) = obj.remove(*key) {
                                                    return Ok(removed);
                                                } else {
                                                    return Err(JsonPathError::PathNotFound(
                                                        format!("Property '{}' not found", key),
                                                    ));
                                                }
                                            } else {
                                                return Err(JsonPathError::NotAnObject(format!(
                                                    "Expected object but found: {:?}",
                                                    target
                                                )));
                                            }
                                        }
                                        _ => {
                                            return Err(JsonPathError::InvalidPath(
                                                "Expected property name".to_string(),
                                            ))
                                        }
                                    }
                                }

                                // For longer paths, navigate to the parent and then remove the last segment
                                let parent_segments = get_parent_segments(segments)?;
                                let last_segment = segments.last().unwrap();

                                let parent = traverse_mut(target, parent_segments)?;

                                match last_segment {
                                    PathSegment::Property(key) => {
                                        if let Value::Object(obj) = parent {
                                            if let Some(removed) = obj.remove(*key) {
                                                Ok(removed)
                                            } else {
                                                Err(JsonPathError::PathNotFound(format!(
                                                    "Property '{}' not found",
                                                    key
                                                )))
                                            }
                                        } else {
                                            Err(JsonPathError::NotAnObject(format!(
                                                "Expected object but found: {:?}",
                                                parent
                                            )))
                                        }
                                    }

                                    PathSegment::Index(idx) => {
                                        if let Value::Array(arr) = parent {
                                            let normalized_idx = normalize_index(*idx, arr.len())?;
                                            Ok(arr.remove(normalized_idx))
                                        } else {
                                            Err(JsonPathError::NotAnArray(format!(
                                                "Expected array but found: {:?}",
                                                parent
                                            )))
                                        }
                                    }

                                    _ => Err(JsonPathError::InvalidPath(
                                        "Invalid last segment".to_string(),
                                    )),
                                }
                            };

                        // Remove from each element in range
                        for i in start_idx..end_idx {
                            if i < arr.len() {
                                match remove_from_target(&mut arr[i], remaining_segments) {
                                    Ok(removed) => removed_values.push(removed),
                                    Err(_) => {} // Skip elements that don't have the property
                                }
                            }
                        }

                        Ok(Value::Array(removed_values))
                    } else {
                        unreachable!()
                    }
                } else {
                    Err(JsonPathError::NotAnArray(format!(
                        "Expected array for range operation but found: {:?}",
                        parent
                    )))
                }
            }
        }
        Err(_) => {
            // No range segment - regular remove operation
            let parent_segments = get_parent_segments(&segments)?;
            let last_segment = segments.last().unwrap();

            let parent = traverse_mut(json, parent_segments)?;

            match last_segment {
                PathSegment::Root => unreachable!(),

                PathSegment::Property(key) => {
                    if let Value::Object(obj) = parent {
                        if let Some(removed) = obj.remove(*key) {
                            Ok(removed)
                        } else {
                            Err(JsonPathError::PathNotFound(format!(
                                "Property '{}' not found",
                                key
                            )))
                        }
                    } else {
                        Err(JsonPathError::NotAnObject(format!(
                            "Parent is not an object: {:?}",
                            parent
                        )))
                    }
                }

                PathSegment::Index(idx) => {
                    if let Value::Array(arr) = parent {
                        let normalized_idx = normalize_index(*idx, arr.len())?;
                        Ok(arr.remove(normalized_idx))
                    } else {
                        Err(JsonPathError::NotAnArray(format!(
                            "Parent is not an array: {:?}",
                            parent
                        )))
                    }
                }

                PathSegment::Range(_, _) => unreachable!(),
            }
        }
    }
}

/// Apply a function to each element in a range of an array
pub fn apply<F>(json: &Value, path: &str, f: F) -> Result<Value, JsonPathError>
where
    F: Fn(&Value) -> Result<Value, JsonPathError>,
{
    let segments = parse_path(path)?;

    // Find the range segment
    match find_range_segment(&segments) {
        Ok((parent_segments_opt, range_idx, remaining_segments)) => {
            // Get a copy of the value to modify
            let mut result = json.clone();

            // Get parent of the range segment
            let parent_segments = parent_segments_opt.unwrap_or(&[]);
            let parent = if parent_segments.is_empty() {
                &mut result
            } else {
                traverse_mut(&mut result, parent_segments)?
            };

            // Function to apply transformation at target path
            let mut apply_func = |target: &mut Value, path_segments: &[PathSegment]| {
                if path_segments.is_empty() {
                    // No remaining path, apply directly
                    *target = f(target)?;
                    Ok(())
                } else {
                    // More path to traverse
                    let target_ref = traverse(target, path_segments)?;
                    let new_value = f(target_ref)?;
                    let target_mut = traverse_mut(target, path_segments)?;
                    *target_mut = new_value;
                    Ok(())
                }
            };

            // Process the range
            if let Value::Array(arr) = parent {
                process_range(
                    arr,
                    &segments[range_idx],
                    remaining_segments,
                    &mut apply_func,
                )?;
                Ok(result)
            } else {
                Err(JsonPathError::NotAnArray(format!(
                    "Expected array for range operation but found: {:?}",
                    parent
                )))
            }
        }
        Err(_) => Err(JsonPathError::InvalidPath(
            "No range segment found in path".to_string(),
        )),
    }
}

/// Apply a mutable function to each element in a range of an array
pub fn apply_mut<F>(json: &mut Value, path: &str, mut f: F) -> Result<(), JsonPathError>
where
    F: FnMut(&mut Value) -> Result<(), JsonPathError>,
{
    let segments = parse_path(path)?;

    // Find the range segment
    match find_range_segment(&segments) {
        Ok((parent_segments_opt, range_idx, remaining_segments)) => {
            // Get parent of the range segment
            let parent_segments = parent_segments_opt.unwrap_or(&[]);
            let parent = if parent_segments.is_empty() {
                json
            } else {
                traverse_mut(json, parent_segments)?
            };

            // Function to apply transformation at target path
            let mut apply_func = |target: &mut Value, path_segments: &[PathSegment]| {
                if path_segments.is_empty() {
                    // No remaining path, apply directly
                    f(target)
                } else {
                    // More path to traverse
                    let target_mut = traverse_mut(target, path_segments)?;
                    f(target_mut)
                }
            };

            // Process the range
            if let Value::Array(arr) = parent {
                process_range(
                    arr,
                    &segments[range_idx],
                    remaining_segments,
                    &mut apply_func,
                )
            } else {
                Err(JsonPathError::NotAnArray(format!(
                    "Expected array for range operation but found: {:?}",
                    parent
                )))
            }
        }
        Err(_) => Err(JsonPathError::InvalidPath(
            "No range segment found in path".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn test_parse_path() {
        assert_eq!(parse_path("/").unwrap(), vec![PathSegment::Root]);

        assert_eq!(
            parse_path("/users").unwrap(),
            vec![PathSegment::Root, PathSegment::Property("users")]
        );

        assert_eq!(
            parse_path("/users[0]").unwrap(),
            vec![
                PathSegment::Root,
                PathSegment::Property("users"),
                PathSegment::Index(0)
            ]
        );

        assert_eq!(
            parse_path("/users[-1]/name").unwrap(),
            vec![
                PathSegment::Root,
                PathSegment::Property("users"),
                PathSegment::Index(-1),
                PathSegment::Property("name")
            ]
        );

        assert_eq!(
            parse_path("/config/logging/level").unwrap(),
            vec![
                PathSegment::Root,
                PathSegment::Property("config"),
                PathSegment::Property("logging"),
                PathSegment::Property("level")
            ]
        );
    }

    #[test]
    fn test_get() {
        let data = json!({
            "name": "Test",
            "arr": [1, 2, 3],
            "obj": {
                "nested": "value",
                "items": ["a", "b", "c"]
            }
        });

        assert_eq!(get(&data, "/name").unwrap(), &json!("Test"));
        assert_eq!(get(&data, "/arr[0]").unwrap(), &json!(1));
        assert_eq!(get(&data, "/arr[-1]").unwrap(), &json!(3));
        assert_eq!(get(&data, "/obj/nested").unwrap(), &json!("value"));
        assert_eq!(get(&data, "/obj/items[1]").unwrap(), &json!("b"));
    }

    #[test]
    fn test_set() {
        let mut data = json!({
            "name": "Test",
            "arr": [1, 2, 3],
            "obj": {
                "nested": "value"
            }
        });

        set(&mut data, "/name", json!("Updated")).unwrap();
        assert_eq!(data["name"], json!("Updated"));

        set(&mut data, "/arr[1]", json!(42)).unwrap();
        assert_eq!(data["arr"][1], json!(42));

        set(&mut data, "/obj/nested", json!("new value")).unwrap();
        assert_eq!(data["obj"]["nested"], json!("new value"));

        set(&mut data, "/obj/new_prop", json!(true)).unwrap();
        assert_eq!(data["obj"]["new_prop"], json!(true));
    }

    #[test]
    fn test_insert() {
        let mut data = json!({
            "arr": [1, 2, 4]
        });

        insert(&mut data, "/arr[2]", json!(3)).unwrap();
        assert_eq!(data["arr"], json!([1, 2, 3, 4]));

        // Insert at the end of the array using length as index
        insert(&mut data, "/arr[4]", json!(5)).unwrap();
        assert_eq!(data["arr"], json!([1, 2, 3, 4, 5]));

        insert(&mut data, "/new_obj", json!({"key": "value"})).unwrap();
        assert_eq!(data["new_obj"], json!({"key": "value"}));
    }

    #[test]
    fn test_remove() {
        let mut data = json!({
            "name": "Test",
            "arr": [1, 2, 3],
            "obj": {
                "a": 1,
                "b": 2
            }
        });

        let removed = remove(&mut data, "/name").unwrap();
        assert_eq!(removed, json!("Test"));
        assert!(data.get("name").is_none());

        let removed = remove(&mut data, "/arr[1]").unwrap();
        assert_eq!(removed, json!(2));
        assert_eq!(data["arr"], json!([1, 3]));

        let removed = remove(&mut data, "/obj/b").unwrap();
        assert_eq!(removed, json!(2));
        assert!(data["obj"].get("b").is_none());
    }

    #[test]
    fn test_remove_range() {
        // Test removing elements with range operator as last segment
        let mut data = json!({
            "numbers": [1, 2, 3, 4, 5],
            "letters": ["a", "b", "c", "d", "e"]
        });

        // Test removing a specific range [1..4]
        let removed = remove(&mut data, "/numbers[1..4]").unwrap();
        assert_eq!(removed, json!([2, 3, 4]));
        assert_eq!(data["numbers"], json!([1, 5]));

        // Test removing from start to an index [..3]
        let removed = remove(&mut data, "/letters[..3]").unwrap();
        assert_eq!(removed, json!(["a", "b", "c"]));
        assert_eq!(data["letters"], json!(["d", "e"]));

        // Test removing from an index to the end [1..]
        let mut data = json!({
            "values": [10, 20, 30, 40, 50]
        });
        let removed = remove(&mut data, "/values[1..]").unwrap();
        assert_eq!(removed, json!([20, 30, 40, 50]));
        assert_eq!(data["values"], json!([10]));
    }

    #[test]
    fn test_remove_with_range_in_middle() {
        // Test removing properties from all elements in an array
        let mut data = json!({
            "messages": [
                {"id": 1, "info": {"type": "A", "status": "active"}},
                {"id": 2, "info": {"type": "B", "status": "pending"}},
                {"id": 3, "info": {"type": "A", "status": "inactive"}},
                {"id": 4}, // Missing info field
                {"id": 5, "info": {"type": "C", "status": "active"}}
            ]
        });

        // Remove "info" from all messages
        let removed = remove(&mut data, "/messages[..]/info").unwrap();

        // Check that the info fields were removed
        assert_eq!(
            data,
            json!({
                "messages": [
                    {"id": 1},
                    {"id": 2},
                    {"id": 3},
                    {"id": 4},
                    {"id": 5}
                ]
            })
        );

        // Check the returned removed values
        assert_eq!(
            removed,
            json!([
                {"type": "A", "status": "active"},
                {"type": "B", "status": "pending"},
                {"type": "A", "status": "inactive"},
                {"type": "C", "status": "active"}
            ])
        );

        // Test with specific range
        let mut data2 = json!({
            "users": [
                {"id": 1, "profile": {"name": "Alice"}},
                {"id": 2, "profile": {"name": "Bob"}},
                {"id": 3, "profile": {"name": "Charlie"}},
                {"id": 4, "profile": {"name": "Dave"}},
                {"id": 5, "profile": {"name": "Eve"}}
            ]
        });

        // Remove profile from users 1-3 only
        let removed2 = remove(&mut data2, "/users[1..4]/profile").unwrap();

        // Check that only those profiles were removed
        assert_eq!(
            data2,
            json!({
                "users": [
                    {"id": 1, "profile": {"name": "Alice"}},
                    {"id": 2},
                    {"id": 3},
                    {"id": 4},
                    {"id": 5, "profile": {"name": "Eve"}}
                ]
            })
        );

        // Check the returned removed values
        assert_eq!(
            removed2,
            json!([
                {"name": "Bob"},
                {"name": "Charlie"},
                {"name": "Dave"}
            ])
        );

        // Test removing a deeper nested property using the pattern '/a[..]/b/c'
        let mut data3 = json!({
            "posts": [
                {
                    "id": 1,
                    "metadata": {
                        "published": true,
                        "stats": {
                            "views": 100,
                            "likes": 15
                        }
                    }
                },
                {
                    "id": 2,
                    "metadata": {
                        "published": false,
                        "stats": {
                            "views": 50,
                            "likes": 5
                        }
                    }
                },
                {
                    "id": 3,
                    "metadata": {
                        "published": true,
                        "stats": {
                            "views": 200,
                            "likes": 32
                        }
                    }
                }
            ]
        });

        // Remove the "likes" count from all posts' stats
        let removed3 = remove(&mut data3, "/posts[..]/metadata/stats/likes").unwrap();

        // Check that the likes property was removed from all posts
        for post in data3["posts"].as_array().unwrap() {
            assert!(post["metadata"]["stats"].get("likes").is_none());
            assert!(post["metadata"]["stats"].get("views").is_some()); // views should still be there
        }

        // Check the returned removed values - should be the like counts
        assert_eq!(removed3, json!([15, 5, 32]));
    }

    #[test]
    fn test_apply() {
        let data = json!({
            "numbers": [1, 2, 3, 4, 5]
        });

        // Test with explicit range [1..4]
        let result = apply(&data, "/numbers[1..4]", |value| {
            if let Value::Number(n) = value {
                if let Some(i) = n.as_i64() {
                    return Ok(json!(i * 2));
                }
            }
            Err(JsonPathError::OperationFailed("Not a number".to_string()))
        })
        .unwrap();

        assert_eq!(result["numbers"], json!([1, 4, 6, 8, 5]));

        // Test with open-ended range [2..]
        let result = apply(&data, "/numbers[2..]", |value| {
            if let Value::Number(n) = value {
                if let Some(i) = n.as_i64() {
                    return Ok(json!(i + 10));
                }
            }
            Err(JsonPathError::OperationFailed("Not a number".to_string()))
        })
        .unwrap();

        assert_eq!(result["numbers"], json!([1, 2, 13, 14, 15]));
    }

    #[test]
    fn test_apply_mut() {
        let mut data = json!({
            "numbers": [1, 2, 3, 4, 5]
        });

        // Test with explicit range [1..4]
        apply_mut(&mut data, "/numbers[1..4]", |value| {
            if let Value::Number(n) = value {
                if let Some(i) = n.as_i64() {
                    *value = json!(i * 2);
                    return Ok(());
                }
            }
            Err(JsonPathError::OperationFailed("Not a number".to_string()))
        })
        .unwrap();

        assert_eq!(data["numbers"], json!([1, 4, 6, 8, 5]));

        // Test with full range [..]
        apply_mut(&mut data, "/numbers[..]", |value| {
            if let Value::Number(n) = value {
                if let Some(i) = n.as_i64() {
                    *value = json!(i + 1);
                    return Ok(());
                }
            }
            Err(JsonPathError::OperationFailed("Not a number".to_string()))
        })
        .unwrap();

        assert_eq!(data["numbers"], json!([2, 5, 7, 9, 6]));
    }

    #[test]
    fn test_set_with_range() {
        // Test setting with range at the end of the path
        let mut data = json!({
            "numbers": [1, 2, 3, 4, 5]
        });

        // Set all elements in range [1..4] to 100
        set(&mut data, "/numbers[1..4]", json!(100)).unwrap();
        assert_eq!(data["numbers"], json!([1, 100, 100, 100, 5]));

        // Test with range in the middle of path
        let mut complex_data = json!({
            "users": [
                {"name": "Alice", "scores": [10, 20, 30]},
                {"name": "Bob", "scores": [15, 25, 35]},
                {"name": "Charlie", "scores": [5, 15, 25]}
            ]
        });

        // Set scores property for users 0 and 1
        set(&mut complex_data, "/users[0..2]/scores[1]", json!(99)).unwrap();

        // Verify the changes
        assert_eq!(complex_data["users"][0]["scores"][1], json!(99));
        assert_eq!(complex_data["users"][1]["scores"][1], json!(99));
        assert_eq!(complex_data["users"][2]["scores"][1], json!(15)); // Unchanged
    }

    #[test]
    fn test_insert_with_range() {
        // Test with range in the middle of path
        let mut data = json!({
            "teams": [
                {"name": "Team A", "members": ["Alice", "Bob"]},
                {"name": "Team B", "members": ["Charlie", "Dave"]},
                {"name": "Team C", "members": ["Eve", "Frank"]}
            ]
        });

        // Add a new member to the first two teams
        insert(&mut data, "/teams[0..2]/members[1]", json!("New Member")).unwrap();

        // Verify members were inserted at index 1 in teams 0 and 1 only
        assert_eq!(
            data["teams"][0]["members"],
            json!(["Alice", "New Member", "Bob"])
        );
        assert_eq!(
            data["teams"][1]["members"],
            json!(["Charlie", "New Member", "Dave"])
        );
        assert_eq!(data["teams"][2]["members"], json!(["Eve", "Frank"])); // Unchanged
    }

    #[test]
    fn test_deep_nested_paths_with_ranges() {
        // Create a complex nested structure with arrays at multiple levels
        let mut data = json!({
            "a": {
                "b": [
                    {
                        "c": [
                            {"d": 1, "e": 10},
                            {"d": 2, "e": 20}
                        ]
                    },
                    {
                        "c": [
                            {"d": 3, "e": 30},
                            {"d": 4, "e": 40}
                        ]
                    },
                    {
                        "c": [
                            {"d": 5, "e": 50},
                            {"d": 6, "e": 60}
                        ]
                    }
                ]
            }
        });

        // Test SET with range at deeper level
        set(&mut data, "/a/b[0..3]/c[0]/d", json!(200)).unwrap();
        assert_eq!(data["a"]["b"][0]["c"][0]["d"], json!(200));
        assert_eq!(data["a"]["b"][1]["c"][0]["d"], json!(200));
        assert_eq!(data["a"]["b"][2]["c"][0]["d"], json!(200));

        // Test with nested structure - set each path individually
        for b_idx in 0..3 {
            for c_idx in 0..2 {
                set(
                    &mut data,
                    &format!("/a/b[{}]/c[{}]/e", b_idx, c_idx),
                    json!(999),
                )
                .unwrap();
            }
        }

        // Verify all values were set
        assert_eq!(data["a"]["b"][0]["c"][0]["e"], json!(999));
        assert_eq!(data["a"]["b"][0]["c"][1]["e"], json!(999));
        assert_eq!(data["a"]["b"][1]["c"][0]["e"], json!(999));
        assert_eq!(data["a"]["b"][1]["c"][1]["e"], json!(999));
        assert_eq!(data["a"]["b"][2]["c"][0]["e"], json!(999));
        assert_eq!(data["a"]["b"][2]["c"][1]["e"], json!(999));
    }

    #[test]
    fn test_composable_operations() {
        // Test that a complex path with various components works as expected
        let mut data = json!({
            "a": {
                "b": [
                    {"c": 1},
                    {"c": 2},
                    {"c": 3}
                ]
            }
        });

        // Complex path with a range in the middle
        insert(&mut data, "/a/b[..]/c", json!(42)).unwrap();
        assert_eq!(data["a"]["b"][0]["c"], json!(42));
        assert_eq!(data["a"]["b"][1]["c"], json!(42));
        assert_eq!(data["a"]["b"][2]["c"], json!(42));

        // Path with negative index
        set(&mut data, "/a/b[-1]/c", json!(100)).unwrap();
        assert_eq!(data["a"]["b"][2]["c"], json!(100));

        // Test with deeper nesting
        let mut complex = json!({
            "departments": [
                {
                    "name": "Engineering",
                    "teams": [
                        {"name": "Frontend", "members": [{"id": 1}, {"id": 2}]},
                        {"name": "Backend", "members": [{"id": 3}, {"id": 4}]}
                    ]
                },
                {
                    "name": "Marketing",
                    "teams": [
                        {"name": "Content", "members": [{"id": 5}, {"id": 6}]},
                        {"name": "Social", "members": [{"id": 7}, {"id": 8}]}
                    ]
                }
            ]
        });

        // Very complex path with multiple ranges
        // Apply operation to each department, then to each team, then to each member
        // First add active field to all members
        let departments = complex["departments"].as_array().unwrap();

        for dept_idx in 0..departments.len() {
            let dept_path = format!("/departments[{}]", dept_idx);
            let teams = complex["departments"][dept_idx]["teams"]
                .as_array()
                .unwrap();

            // For each team, apply to members
            for team_idx in 0..teams.len() {
                let team_path = format!("{}/teams[{}]", dept_path, team_idx);
                let members = complex["departments"][dept_idx]["teams"][team_idx]["members"]
                    .as_array()
                    .unwrap();

                // For each member, add active field
                for member_idx in 0..members.len() {
                    let member_path = format!("{}/members[{}]/active", team_path, member_idx);
                    set(&mut complex, &member_path, json!(true)).unwrap();
                }
            }
        }

        // Verify that every member in every team in every department has the active flag
        for dept in complex["departments"].as_array().unwrap() {
            for team in dept["teams"].as_array().unwrap() {
                for member in team["members"].as_array().unwrap() {
                    assert_eq!(member["active"], json!(true));
                }
            }
        }
    }
}
