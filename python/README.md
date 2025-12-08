# Rhythm Python Quickstart

Install Rhythm
```bash
pip install rhythm-async
```

Setup the example project
```bash
git clone https://github.com/maxnorth/rhythm.git
cd rhythm/python/examples/quickstart
docker compose up -d postgres
```

Start the worker
```bash
python worker.py
```

In another terminal, run the client app
```bash
python app.py
```

## Documentation

See the [examples](examples/) directory for complete working examples.
