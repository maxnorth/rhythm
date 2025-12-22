# Rhythm Python Quickstart

Install Rhythm
```bash
pip install rhythm-py
```

Setup the example project and start Postgres
```bash
git clone https://github.com/maxnorth/rhythm.git
cd rhythm
docker compose up -d postgres
```

Start the worker
```bash
cd rhythm/python/examples/quickstart
python worker.py
```

In another terminal, run the client app
```bash
cd rhythm/python/examples/quickstart
python app.py
```

## Documentation

See the [examples](examples/) directory for complete working examples.
