# Security policy

## Reporting a vulnerability

Please **do not open a public issue** for a security problem. Report it privately with
GitHub's form: **Security → Report a vulnerability**
(<https://github.com/wvmscs/geohash-lookup/security/advisories/new>).

Say what you found, how to reproduce it, and what an attacker gains. A fix and an advisory
follow, crediting you if you wish.

## What counts

Anything that breaks what this project promises its users, for example:

- a query, an IP address looked up or a result that reaches a log, a file or a service it
  should not (IP addresses are looked up locally only);
- a crafted answer from BAN, Nominatim or DB-IP, or a crafted GeoIP file, that crashes the
  program, runs code, writes outside the path given, or fills the disk or the memory;
- a GeoIP download that is used without being checked.

Weaknesses of the services themselves (BAN, Nominatim, DB-IP) belong to their operators, but
tell us too if they affect this project.

## Supported versions

The latest release tag and the `main` branch.
