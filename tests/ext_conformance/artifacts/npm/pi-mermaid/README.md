# 🧜‍♀️ pi-mermaid

Pi extension that renders Mermaid diagrams as ASCII in the TUI.

## Features
- Renders Mermaid blocks as ASCII diagrams inside Pi's TUI
- Collapsible output with source shown only on expand (ctrl+o)
- Token efficiently adds parser warnings/errors to LLM context
- Handles large blocks with safety limits and caching

## Install

```bash
pi install npm:pi-mermaid
```

Or, clone into your Pi extensions directory and enable it:

```bash
git clone https://github.com/Gurpartap/pi-mermaid ~/.kode/agent/extensions/pi-mermaid
```

After installing, enter `/reload` or restart Pi.

## Usage
Use Mermaid fenced blocks in chat:

````
```mermaid
graph TD
  Start --> End
```
````

Or render the last assistant message:

```
/pi-mermaid
```

## Examples
More examples: https://agents.craft.do/mermaid

````
Mermaid (ASCII)
 ┌───────┐     ┌────────┐     ┌──────────┐
 │       │     │        │     │          │
 │ Start ├────►│ Choice ├─Yes►│ Do thing │
 │       │     │        │     │          │
 └───────┘     └────┬───┘     └──────────┘
                    │
                    │
                    │
                   No
                    │
                    │         ┌──────────┐
                    │         │          │
                    └────────►│   Skip   │
                              │          │
                              └──────────┘

```mermaid
flowchart LR
  A[Start] --> B{Choice}
  B -->|Yes| C[Do thing]
  B -->|No| D[Skip]
```
````

````
Mermaid (ASCII)
 ┌──────┐           ┌─────┐           ┌─────┐
 │ User │           │ Web │           │ API │
 └───┬──┘           └──┬──┘           └──┬──┘
     │                 │                 │
     │   Submit form   │                 │
     │─────────────────▶                 │
     │                 │                 │
     │                 │  POST /submit   │
     │                 │─────────────────▶
     │                 │                 │
     │                 │   201 Created   │
     │                 ◀╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌│
     │                 │                 │
     │  Show success   │                 │
     ◀╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌│                 │
     │                 │                 │
 ┌───┴──┐           ┌──┴──┐           ┌──┴──┐
 │ User │           │ Web │           │ API │
 └──────┘           └─────┘           └─────┘

```mermaid
sequenceDiagram
  participant U as User
  participant W as Web
  participant API as API
  U->>W: Submit form
  W->>API: POST /submit
  API-->>W: 201 Created
  W-->>U: Show success
```
````

````
Mermaid (ASCII)
 ┌────────┐      ┌─────┐                  ┌──────────┐
 │ Client │      │ API │                  │ Database │
 └────┬───┘      └──┬──┘                  └─────┬────┘
      │             │                           │
      │  transfer   │                           │
      │─────────────▶                           │
      │             │                           │
      │             │  txn (debit/credit/log)   │
      │             │───────────────────────────▶
      │             │                           │
  ┌alt [ok]─────────────────────────────────────────┐
  │   │             │                           │   │
  │   │             │          commit           │   │
  │   │             │───────────────────────────▶   │
  │   │             │                           │   │
  │   │     200     │                           │   │
  │   ◀╌╌╌╌╌╌╌╌╌╌╌╌╌│                           │   │
  │   │             │                           │   │
  ├[fail]╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
  │   │             │                           │   │
  │   │             │         rollback          │   │
  │   │             │───────────────────────────▶   │
  │   │             │                           │   │
  │   │     400     │                           │   │
  │   ◀╌╌╌╌╌╌╌╌╌╌╌╌╌│                           │   │
  │   │             │                           │   │
  └─────────────────────────────────────────────────┘
      │             │                           │
 ┌────┴───┐      ┌──┴──┐                  ┌─────┴────┐
 │ Client │      │ API │                  │ Database │
 └────────┘      └─────┘                  └──────────┘

```mermaid
sequenceDiagram
  participant Client
  participant API
  participant Database
  Client->>API: transfer
  API->>Database: txn (debit/credit/log)
  alt ok
    API->>Database: commit
    API-->>Client: 200
  else fail
    API->>Database: rollback
    API-->>Client: 400
  end
```
````

````
Mermaid (ASCII)
  ┌─────┐            ┌───────┐  ┌──────────┐
  │ App │            │ Cache │  │ Database │
  └──┬──┘            └───┬───┘  └─────┬────┘
     │                   │            │
     │     Get data      │            │
     │───────────────────▶            │
     │                   │            │
     │    Cache miss     │            │
     ◀╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌│            │
     │                   │            │
 ┌opt [Cache miss]────────────────────────┐
 │   │                   │            │   │
 │   │             Query │            │   │
 │   │────────────────────────────────▶   │
 │   │                   │            │   │
 │   │            Results│            │   │
 │   ◀╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌│   │
 │   │                   │            │   │
 │   │  Store in cache   │            │   │
 │   │───────────────────▶            │   │
 │   │                   │            │   │
 └────────────────────────────────────────┘
     │                   │            │
  ┌──┴──┐            ┌───┴───┐  ┌─────┴────┐
  │ App │            │ Cache │  │ Database │
  └─────┘            └───────┘  └──────────┘

```mermaid
sequenceDiagram
  participant A as App
  participant C as Cache
  participant DB as Database
  A->>C: Get data
  C-->>A: Cache miss
  opt Cache miss
    A->>DB: Query
    DB-->>A: Results
    A->>C: Store in cache
  end
```
````

````
Mermaid (ASCII)
 ┌───────────────┐
 │ Animal        │
 ├───────────────┤
 │ +name: String │
 ├───────────────┤
 │ +eat: void    │
 └───────────────┘
         △
         └──────────────────────┐
          │                     │
 ┌────────────────┐    ┌─────────────────┐
 │ Dog            │    │ Cat             │
 ├────────────────┤    ├─────────────────┤
 │ +breed: String │    │ +isIndoor: bool │
 ├────────────────┤    ├─────────────────┤
 │ +bark: void    │    │ +meow: void     │
 └────────────────┘    └─────────────────┘

```mermaid
classDiagram
  class Animal {
    +String name
    +eat() void
  }
  class Dog {
    +String breed
    +bark() void
  }
  class Cat {
    +bool isIndoor
    +meow() void
  }
  Animal <|-- Dog
  Animal <|-- Cat
```
````

## License
MIT © 2026 Gurpartap Singh (https://x.com/Gurpartap)
