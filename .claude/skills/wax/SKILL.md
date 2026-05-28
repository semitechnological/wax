```markdown
# wax Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches you the core development patterns and conventions used in the `wax` Rust codebase. You'll learn about file naming, import/export styles, commit message habits, and how to run and write tests, even though no formal workflows or frameworks are detected. This guide helps ensure consistency and efficiency when contributing to `wax`.

## Coding Conventions

### File Naming
- Use **camelCase** for file names.
  - Example: `myModule.rs`, `dataParser.rs`

### Import Style
- Use **relative imports** within the codebase.
  - Example:
    ```rust
    mod utils;
    use crate::utils::parseData;
    ```

### Export Style
- Use **named exports** to expose functions, structs, or modules.
  - Example:
    ```rust
    pub fn processData() { /* ... */ }
    pub struct WaxItem { /* ... */ }
    ```

### Commit Messages
- No strict prefix or type; freeform style.
- Average commit message length: ~33 characters.
  - Example: `fix bug in data parser`

## Workflows

### Adding a New Module
**Trigger:** When you need to add new functionality.
**Command:** `/add-module`

1. Create a new file using camelCase naming (e.g., `myFeature.rs`).
2. Implement your module logic.
3. Use relative imports to include dependencies.
4. Export your functions/structs using `pub`.
5. Update the main module or lib to include your new module.

### Writing and Running Tests
**Trigger:** When you want to test new or existing code.
**Command:** `/run-tests`

1. Create a test file with the pattern `*.test.*` (e.g., `parser.test.rs`).
2. Write tests using Rust's built-in testing framework.
   - Example:
     ```rust
     #[cfg(test)]
     mod tests {
         use super::*;

         #[test]
         fn test_parse_data() {
             assert_eq!(parseData("input"), "expected");
         }
     }
     ```
3. Run tests using Cargo:
   ```
   cargo test
   ```

### Making a Commit
**Trigger:** When you have changes ready to commit.
**Command:** `/commit`

1. Stage your changes:
   ```
   git add .
   ```
2. Write a concise, freeform commit message (~33 chars recommended).
   ```
   git commit -m "describe your change"
   ```
3. Push your changes:
   ```
   git push
   ```

## Testing Patterns

- Test files follow the `*.test.*` naming convention (e.g., `math.test.rs`).
- Tests are written using Rust's built-in test framework (`#[cfg(test)]`).
- Example test:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_function() {
          assert_eq!(function(), expected_value);
      }
  }
  ```

## Commands
| Command        | Purpose                                      |
|----------------|----------------------------------------------|
| /add-module    | Scaffold and add a new module                |
| /run-tests     | Run all tests in the codebase                |
| /commit        | Stage, commit, and push your changes         |
```
