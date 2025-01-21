use std::{
    collections::HashMap,
    fs,
    io::{Seek, Write},
};

pub(crate) struct Metrics {
    // used to make sure no transaction/message is delivered twice
    total_delivered: u64,
    // delivered does not include own transactions/messages
    delivered: HashMap<Vec<u8>, u64>, // tx/msg id -> timestamp of delivery
    // published concerns only own transactions/messages
    published: HashMap<Vec<u8>, u64>, // tx/msg id -> timestamp of publication
}

impl Metrics {
    pub(crate) fn new() -> Self {
        Self {
            total_delivered: 0,
            delivered: HashMap::new(),
            published: HashMap::new(),
        }
    }

    pub(crate) fn total_delivered(&self) -> u64 {
        self.total_delivered
    }

    pub(crate) fn add_delivered(&mut self, id: Vec<u8>, timestamp: u64) {
        if let Some(time) = self.delivered.get(&id) {
            tracing::warn!(
                "Transaction {} already delivered {} ms ago",
                match std::str::from_utf8(&id) {
                    Ok(s) => s,
                    Err(_) => "Invalid UTF-8",
                },
                timestamp - time
            );
            return;
        }

        self.total_delivered += 1;
        self.delivered.insert(id, timestamp);
    }

    pub(crate) fn add_published(&mut self, id: Vec<u8>, timestamp: u64) {
        if let Some(time) = self.published.get(&id) {
            tracing::warn!(
                "Transaction {} already published {} ms ago",
                match std::str::from_utf8(&id) {
                    Ok(s) => s,
                    Err(_) => "Invalid UTF-8",
                },
                timestamp - time
            );
            return;
        }

        self.published.insert(id, timestamp);
    }

    pub(crate) fn dump_metrics(
        &self,
        published_path: String,
        delivered_path: String,
    ) -> std::io::Result<()> {
        let published_file = fs::File::create(published_path)?;
        let delivered_file = fs::File::create(delivered_path)?;

        let mut published_writer = std::io::BufWriter::new(&published_file);
        let mut delivered_writer = std::io::BufWriter::new(&delivered_file);

        published_writer.write_all(b"{\"published\":[")?;
        delivered_writer.write_all(b"{\"delivered\":[")?;

        for (id_vec, timestamp) in &self.published {
            match std::str::from_utf8(&id_vec) {
                Ok(id) => {
                    published_writer
                        .write_all(format!("[\"{}\",{}],", id, timestamp).as_bytes())?;
                }
                Err(e) => {
                    tracing::error!("Failed to convert id to string: {:?}", e);
                }
            }
        }

        for (id_vec, timestamp) in &self.delivered {
            match std::str::from_utf8(&id_vec) {
                Ok(id) => {
                    delivered_writer
                        .write_all(format!("[\"{}\",{}],", id, timestamp).as_bytes())?;
                }
                Err(e) => {
                    tracing::error!("Failed to convert id to string: {:?}", e);
                }
            }
        }

        published_writer.seek(std::io::SeekFrom::End(-1))?; // Remove trailing comma
        delivered_writer.seek(std::io::SeekFrom::End(-1))?; // Remove trailing comma

        published_writer.write_all(b"]}")?;
        delivered_writer.write_all(b"]}")?;

        published_writer.flush()?;
        delivered_writer.flush()?;

        Ok(())
    }
}
