use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Mn,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Mn => "mn",
        }
    }

    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "en" | "english" => Some(Lang::En),
            "mn" | "mongolian" => Some(Lang::Mn),
            _ => None,
        }
    }

    pub fn toggle(self) -> Lang {
        match self {
            Lang::En => Lang::Mn,
            Lang::Mn => Lang::En,
        }
    }

    fn list(self) -> &'static str {
        match self {
            Lang::En => EN,
            Lang::Mn => MN,
        }
    }
}

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
pub fn generate(lang: Lang, count: usize, rng: &mut Rng) -> Vec<Vec<char>> {
    let pool: Vec<&str> = lang.list().split_whitespace().collect();
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

const EN: &str = "
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

const MN: &str = "
би чи бид та тэр энэ тэд юу хэн хаана хэзээ яагаад яаж байна байх болно биш үгүй тийм
сайн муу том жижиг шинэ хуучин өдөр шөнө өглөө орой цаг жил сар ус гал газар тэнгэр нар
од салхи бороо цас мод уул гол нуур тал хот гэр байшин сургууль ном хүн эх эцэг ах эгч дүү
найз хүүхэд багш ажил мөнгө хоол цай сүү мах талх морь нохой муур үхэр хонь ямаа тэмээ
машин зам хаалга цонх ширээ сандал үг хэл монгол улс нэр нүд чих гар хөл толгой зүрх
сэтгэл хайр аз жаргал дуу хөгжим тоглоом хурд уралдаан товч бичих унших явах ирэх идэх
уух унтах сурах хийх өгөх авах харах сонсох ярих мэдэх хүсэх чадах гүйх суух босох нэг
хоёр гурав дөрөв тав зургаа долоо найм ес арав зуу мянга улаан цагаан хар хөх ногоон шар
халуун хүйтэн хурдан удаан хол ойр их бага олон цөөн өндөр намхан сайхан гоё амар хэцүү
одоо маргааш өчигдөр өнөөдөр дахин хамт ганц бүх зөвхөн маш бас гэхдээ учир тэгээд эсвэл
хэрэв ба
";
