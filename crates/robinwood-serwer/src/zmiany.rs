//! Zmiany częściowe (PUT z samymi zmienionymi polami) z wykrywaniem konfliktów.
//!
//! Treść żądania to obiekt z polami do zmiany (`null` = wyczyść pole) i opcjonalnym obiektem
//! `poprzednie` – wartościami, które stanowisko widziało przed edycją:
//!
//! ```json
//! { "material": "Płyta B", "poprzednie": { "material": "Płyta A" } }
//! ```
//!
//! Serwer porównuje `poprzednie` z bieżącą wartością w bazie. Jeśli ktoś inny zmienił to samo
//! pole w międzyczasie (bieżąca ≠ poprzednia i ≠ nowa), zwraca 409 z listą konfliktów
//! i aktualnym rekordem. Pola bez wpisu w `poprzednie` są zapisywane bez sprawdzania
//! (ostatni zapis wygrywa), a zmiany innych pól tego samego rekordu nie są konfliktem.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::bledy::{tlumacz_serde, BladApi, KonfliktPola, Wynik};

#[derive(Debug, Clone, Default)]
pub struct ZmianaCzesciowa {
    pub zmiany: Map<String, Value>,
    pub poprzednie: Map<String, Value>,
}

impl ZmianaCzesciowa {
    /// Rozbiera treść żądania; pola spoza `dozwolone` → 422.
    pub fn z_json(v: Value, dozwolone: &[&str]) -> Wynik<Self> {
        let Value::Object(mut m) = v else {
            return Err(BladApi::NieprawidloweDane(
                "treść żądania: oczekiwano obiektu JSON".into(),
            ));
        };
        let poprzednie = match m.remove("poprzednie") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(p)) => p,
            Some(_) => {
                return Err(BladApi::NieprawidloweDane(
                    "pole „poprzednie” musi być obiektem".into(),
                ))
            }
        };
        // Licznik wersji, który klient widział – tylko informacyjnie (konflikty liczymy po polach).
        m.remove("wersja");
        for k in m.keys().chain(poprzednie.keys()) {
            if !dozwolone.contains(&k.as_str()) {
                return Err(BladApi::NieprawidloweDane(format!(
                    "nieznane pole „{k}” (dozwolone: {})",
                    dozwolone.join(", ")
                )));
            }
        }
        Ok(Self {
            zmiany: m,
            poprzednie,
        })
    }

    /// Nakłada zmiany na bieżące dane (serializowane do JSON) i odtwarza strukturę.
    pub fn zastosuj<T: Serialize + DeserializeOwned>(&self, aktualne: &T) -> Wynik<T> {
        let mut v = serde_json::to_value(aktualne)
            .map_err(|e| BladApi::Wewnetrzny(format!("serializacja: {e}")))?;
        if let Value::Object(m) = &mut v {
            for (k, w) in &self.zmiany {
                m.insert(k.clone(), w.clone());
            }
        }
        serde_json::from_value(v).map_err(|e| {
            BladApi::NieprawidloweDane(format!("treść żądania: {}", tlumacz_serde(&e.to_string())))
        })
    }

    /// Konflikty: pola zmienione przez kogoś innego od chwili, gdy stanowisko je odczytało.
    pub fn konflikty(&self, aktualny: &Value) -> Vec<KonfliktPola> {
        let mut wynik = Vec::new();
        for (pole, twoja) in &self.zmiany {
            let Some(poprzednia) = self.poprzednie.get(pole) else {
                continue;
            };
            let biezaca = aktualny.get(pole).cloned().unwrap_or(Value::Null);
            if !rowne(&biezaca, poprzednia) && !rowne(&biezaca, twoja) {
                wynik.push(KonfliktPola {
                    pole: pole.clone(),
                    poprzednia: poprzednia.clone(),
                    twoja: twoja.clone(),
                    aktualna: biezaca,
                });
            }
        }
        wynik
    }

    /// Zwraca błąd 409, jeśli są konflikty.
    pub fn sprawdz(
        &self,
        aktualny: &Value,
        rekord: Value,
        nazwy: &dyn Fn(&str) -> String,
    ) -> Wynik<()> {
        let k = self.konflikty(aktualny);
        if k.is_empty() {
            return Ok(());
        }
        let pola: Vec<String> = k.iter().map(|x| nazwy(&x.pole)).collect();
        Err(BladApi::Konflikt {
            komunikat: format!(
                "Ktoś inny zmienił w międzyczasie: {} – wybierz, którą wartość zachować",
                pola.join(", ")
            ),
            konflikty: k,
            aktualny: rekord,
        })
    }
}

/// Porównanie wartości „jak widzi je użytkownik”: pusty tekst = null, liczby jako f64,
/// tekst bez spacji na końcach.
pub fn rowne(a: &Value, b: &Value) -> bool {
    fn norm(v: &Value) -> Value {
        match v {
            Value::String(s) if s.trim().is_empty() => Value::Null,
            Value::String(s) => Value::String(s.trim().to_string()),
            Value::Number(n) => n
                .as_f64()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Value::Bool(b) => Value::Number(i64::from(*b).into()),
            inne => inne.clone(),
        }
    }
    let (a, b) = (norm(a), norm(b));
    match (&a, &b) {
        (Value::Number(x), Value::Number(y)) => {
            (x.as_f64().unwrap_or(f64::NAN) - y.as_f64().unwrap_or(f64::NAN)).abs() < 1e-9
        }
        _ => a == b,
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use serde_json::json;

    #[test]
    fn konflikt_tylko_tego_samego_pola() {
        let z = ZmianaCzesciowa::z_json(
            json!({ "material": "B", "opis": "x", "poprzednie": { "material": "A", "opis": "" } }),
            &["material", "opis"],
        )
        .unwrap();
        // ktoś zmienił opis na „x” (tak samo) i materiał na „C” → konflikt tylko materiału
        let k = z.konflikty(&json!({ "material": "C", "opis": "x" }));
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].pole, "material");
        // nikt nic nie zmienił
        assert!(z
            .konflikty(&json!({ "material": "A", "opis": null }))
            .is_empty());
        assert!(ZmianaCzesciowa::z_json(json!({ "zle": 1 }), &["material"]).is_err());
        assert!(rowne(&json!(2), &json!(2.0)));
        assert!(rowne(&json!(""), &Value::Null));
    }
}
