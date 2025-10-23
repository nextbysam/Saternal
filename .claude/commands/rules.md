## Core Principle: Modular "Lego" Architecture

Code should be instantly switchable and composable like Lego blocks. Every component should be:
- **Single-purpose**: Each file/function does one thing well
- **Reusable**: Components can be combined in different ways
- **Testable**: Easy to test in isolation
- **Replaceable**: Can swap implementations without breaking dependencies

## File Structure Requirements

### React Projects
- Use custom hooks for all logic extraction
- Component files: max 100 lines
- Hook files: max 50 lines per hook
- Separate concerns: UI, logic, data, styling

```
src/
├── components/          # UI components only
├── hooks/              # Custom hooks for logic
├── services/           # API and external services
├── utils/              # Pure utility functions
├── types/              # TypeScript definitions
└── constants/          # App constants
```

### General Projects
- Max 200 lines per file
- Single responsibility per module
- Clear separation of concerns
- Dependency injection over tight coupling

## Anti-Patterns to Avoid

❌ **Monolithic files** - Everything in one place
❌ **Code duplication** - Copy-pasting existing code
❌ **Mixed concerns** - UI logic mixed with business logic
❌ **Deep nesting** - More than 3 levels of folder depth
❌ **Tight coupling** - Components that can't work independently

## Best Practices

✅ **Compose, don't copy** - Extend existing code instead of duplicating
✅ **Extract early** - Split files before they get large
✅ **Name clearly** - Function/file names should explain purpose
✅ **Layer properly** - Clear boundaries between layers
✅ **Test boundaries** - Each module should be testable

## Implementation Strategy

1. **Identify existing patterns** before writing new code
2. **Extract common logic** into reusable modules
3. **Build on existing foundation** rather than recreating
4. **Refactor incrementally** - improve structure as you go
5. **Document interfaces** between modules

Remember: Good architecture makes future changes easy, not just current features.