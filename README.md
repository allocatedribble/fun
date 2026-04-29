# Fun

Fun is a work-in-progress first person shooter built around tactical movement,
destructible environments, and large-scale combined-arms battles.

## Vision

The game should feel sharp enough for a competitive 5v5 match, but the sandbox is
being designed for much larger battles: three-team, PlanetSide 2-like wars with
roughly 100v100v100 players or more, infantry squads, vehicles, and territory
pressure all fighting over the same physical space.

## Core Pillars

- First person shooter fundamentals: readable movement, responsive weapons, and
  clear tactical decision-making.
- Environment destruction that changes routes, sightlines, cover, objectives,
  and vehicle access during a match.
- Competitive 5v5 mode for tight team play, balance testing, and fast iteration.
- Large three-faction war mode built for 100v100v100-scale battles and beyond.
- Vehicles as part of the battlefield, not a separate minigame: transport, armor,
  fire support, logistics, and objective pressure.
- A shared sandbox across modes so tactics learned in 5v5 still matter in the
  larger war.

## Modes

### Competitive 5v5

A smaller, focused mode for round-to-round mastery. Maps are dense, objectives
are readable, and destruction is constrained enough to keep matches fair while
still allowing teams to reshape the fight.

### Three-Faction War

A large-scale combined-arms mode with three opposing teams fighting over a
persistent battlefield. The target scale is 100v100v100 or larger, with infantry,
ground vehicles, air vehicles, destructible strongholds, supply pressure, and
front lines that move as players break and rebuild control of the environment.

## Development Status

This repository is currently an early Rust/Bevy prototype. The immediate focus is
on first-person movement, collision, rendering, networking foundations, and the
technical groundwork needed for meshlet rendering, raytraced lighting, and future
large-match simulation.
