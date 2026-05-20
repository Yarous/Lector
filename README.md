# Lector

**Lector** (лат. *чтец*) — высокопроизводительная система каскадной раздачи файлов
в локальной сети. Спроектирована для школьных классов и компьютерных лабораторий
с устаревшим оборудованием.

## Архитектура

```
              [lector-ui — Учитель]
               /                \
       [lectord ПК1]        [lectord ПК2]
        /        \            /        \
  [lectord ПК3] [lectord ПК4] [lectord ПК5] [lectord ПК6]
```

Файл передаётся по каскадному бинарному дереву через QUIC (UDP).
Управление — через gRPC. Дерево динамически перестраивается при отказе узлов.

## Компоненты

| Crate | Назначение |
|---|---|
| `lector-proto` | Protobuf-определения и сгенерированный код |
| `lector-transport` | QUIC-стриминг файлов (quinn) |
| `lector-topology` | Построение и перебалансировка дерева |
| `lectord` | Фоновый демон на ПК ученика |
| `lector-ui` | GUI панель управления для учителя |

## Сборка

```bash
cd certs && bash generate.sh && cd ..
cargo build --release
./target/release/lectord --config /etc/lector/config.toml
./target/release/lector-ui
```

## Требования

- Rust 1.80+
- protoc (Protocol Buffers compiler)
- Локальная сеть с открытыми портами 50051 (TCP/gRPC) и 50052 (UDP/QUIC)

## Лицензия

MIT
