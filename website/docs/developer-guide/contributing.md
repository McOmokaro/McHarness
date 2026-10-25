---
id: contributing
title: Contributing
slug: /docs/developer-guide/contributing
sidebar_label: Contributing
pagination:
  prev: /docs/developer-guide/architecture
---
# Contributing

We love contributions! Here's how to get started:

## Development Setup

```bash
# Fork the repository
git clone https://github.com/your-org/mcharness.git
cd mcharness

# Install dependencies
yarn install

# Start the dev server
yarn start
```

## Project Structure

```
src/
  tools/         # Tool implementations
  skills/        # Skill definitions
  memory/        # Memory system
  bots/          # Bot mode configuration
  mcp/           # MCP server integration
  voice/         # Voice mode features
docs/            # This documentation
```

## Code Style

- Follow the existing code style in each directory
- Write tests for new features
- Update documentation for API changes
- Commit messages follow Conventional Commits

## Pull Request Process

1. Create a feature branch from `main`
2. Implement your feature
3. Add/update tests
4. Update documentation
5. Submit the PR

<Search />