# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in spectralint, please report it responsibly.

**Email:** lukas@byallmeans.co

Please include:
- Description of the vulnerability
- Steps to reproduce
- Impact assessment

We aim to respond within 48 hours and will coordinate disclosure with you.

## Scope

spectralint runs entirely locally with no network access. Security concerns are limited to:
- False negatives in `credential-exposure` (missing a real secret)
- False negatives in `prompt-injection-vector` (missing an attack pattern)
- Unexpected behavior when processing maliciously crafted markdown files

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |
