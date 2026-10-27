# Hi There!

Thanks for taking an interest in `spool`. Here's a quick start guide to take off running:

# writing your configuration file

spool.toml is where you'll find your configuration. It's pretty self explanatory.

```toml
address = "0.0.0.0:3000" 
max_file_size_bytes = 2147483648 # two gigabytes! (in bytes)
storage_path = "/home/path/to/your/storage"
```

this should be sufficient for most deployments.

for your .env file, all you need to put is the URL to your SQLite database.

```
DATABASE_URL=sqlite://spool.db?mode=rwc
```

(You can name it whatever you'd like, of course.)

As of **right now** `spool` only supports `SQLite` as a supported database backend
