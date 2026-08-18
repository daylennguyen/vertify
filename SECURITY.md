# Security Policy

## Supported versions

The latest released version of vertify is the only version that receives security fixes.

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Use [GitHub private vulnerability reporting](https://github.com/daylennguyen/vertify/security/advisories/new) on this repository, or email the maintainer through the contact details on [the GitHub profile](https://github.com/daylennguyen).

Include:

- A description of the issue and its impact
- Steps to reproduce, or a proof of concept
- Affected version / commit if you know it

You should hear back within 7 days. If the report is confirmed, a fix will be released as soon as possible and credit will be given unless you ask otherwise.

## Scope

vertify shells out to `ffmpeg` / `ffprobe` with paths and filter graphs derived from CLI flags. Reports that involve command injection, path traversal, or unexpected file overwrite are in scope. Issues in ffmpeg itself should be reported upstream.
