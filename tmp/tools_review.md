# Tools Prompt Review

The tools.hbs template is generally comprehensive and well-structured, but has a few issues to address:

1. **Inconsistent patch tool syntax**:
   - The example shows `<<<<BEFORE` and `<<<<AFTER` but the template uses Handlebars syntax
   - This mismatch could confuse users about the correct syntax

2. **Malformatted interruption section**:
   - An interruption example appears disconnected from the shell tool section
   - Makes it unclear which tool the interruption applies to

3. **MCP tools section needs improvement**:
   - Current implementation looks incomplete with nested loops
   - Format appears inconsistent with other tool documentation

4. **Some tool examples could be more realistic**:
   - Current examples are generic and might benefit from more contextual usage

5. **Task tool listing**:
   - Has a placeholder for agent kinds that could be more descriptive

These issues should be addressed in the updated version.