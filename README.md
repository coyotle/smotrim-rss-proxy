# RSS прокси smotrim.ru

Прокси сервер на лету преобразует JSON ответы REST-сервера smotrim.ru в XML/RSS формат.

```sh
Usage: smotrim-rss-proxy [OPTIONS]

Options:
  -i, --ip <IP>
          IP для запуска сервера [default: 127.0.0.1]
  -p, --port <PORT>
          TCP порт сервера [default: 3000]
  -l, --limit <LIMIT>
          Количество эпизодов [default: 20]
  -c, --cache-lifetime <CACHE_LIFETIME>
          Время жизни кэша в секундах [default: 600]
  -d, --db-path <DB_PATH>
          Путь к sqlite базе для хранения данных [default: data.sqlite]
  -h, --help
          Print help
  -V, --version
          Print version
```

```sh
./smotrim-rss-proxy
Server running at http://127.0.0.1:3000
```

После старта сервера можно открывать адреса вида: http://127.0.0.3000/brand/<id>
где id - идентификатор подкаста на платформе "Смотрим".
