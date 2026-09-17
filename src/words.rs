use std::time::{SystemTime, UNIX_EPOCH};

/// xorshift64 -- plenty for shuffling words, and saves pulling in `rand`
pub struct Rng(u64);

impl Rng {
    pub fn seeded() -> Rng {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Rng(nanos | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// `count` random words, never the same word twice in a row
pub fn generate(count: usize, rng: &mut Rng) -> Vec<Vec<char>> {
    let pool: Vec<&str> = WORDS.split_whitespace().collect();
    let mut out: Vec<Vec<char>> = Vec::with_capacity(count);
    let mut last = usize::MAX;
    while out.len() < count {
        let i = rng.below(pool.len());
        if i == last {
            continue;
        }
        last = i;
        out.push(pool[i].chars().collect());
    }
    out
}

const WORDS: &str = "
the be of and a to in he have it that for they i with as not on she at by this we you
do but from or which one would all will there say who make when can more if no man out
other so what time up go about than into could state only new year some take come these
know see use get like then first any work now may such give over think most even find day
also after way many must look before great back through long where much should well people
down own just because good each those feel seem how high too place little world very still
nation hand old life tell write become here show house both between need mean call develop
under last right move thing general school never same another begin while number part turn
real leave might want point form off child few small since against ask late home interest
large person end open public follow during present without again hold govern around possible
head consider word program problem however lead system set order eye plan run keep face fact
group play stand increase early course change help line
";
