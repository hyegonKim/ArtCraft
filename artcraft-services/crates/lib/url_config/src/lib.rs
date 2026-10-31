/*!

This library is meant to handle the following URL construction cases safely:

  - Determining cookie domains to set (environmental, hostname contextual)
  - Hardcoded variable redirects (environmental, hostname contextual)
  - Safe web redirects (from user params!!)

  - Stripe redirects/backlinks:
    - Stripe checkout:
      - Success URL (environmental, hostname contextual)
      - Cancel URL (environmental, hostname contextual, maybe path/state contextual?)
    - Stripe customer portal
      - Return page (environmental, hostname contextual, maybe path/state contextual?)

   - Twitch:
     - OAuth redirect landing (environmental, hostname contextual)

Here are the domain names we handle:

  - Frontend Production:
    - fakeyou.com
    - storyteller.io
    - storyteller.stream
    - obs.storyteller.stream

  - Frontend Development:
    - localhost (various ports)
    - jungle.horse
    - dev.fakeyou.com
    - dev-proxy.fakeyou.com - sends traffic to production (since there's no staging yet)

  - Development API:
    - localhost (various ports)
    - api.jungle.horse
    - api.dev.fakeyou.com

*/

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]
#![forbid(unreachable_patterns)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

pub mod third_party_url_redirector;
