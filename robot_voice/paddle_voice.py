import re

import cn2an
import cmudict
import numpy as np
import onnxruntime as ort
from pypinyin import Style
from pypinyin.core import Pinyin
from pypinyin.contrib.tone_sandhi import ToneSandhiMixin
from pypinyin.converter import DefaultConverter


class MandarinConverter(ToneSandhiMixin, DefaultConverter):
    pass


class PaddleVoice:
    """使用同一说话人的中英音素序列合成整句。"""

    def __init__(self, models):
        acoustic = models / "fastspeech2_mix_onnx_0.2.0"
        vocoder = models / "hifigan_csmsc_onnx_0.2.0"
        opts = ort.SessionOptions()
        opts.intra_op_num_threads = 3
        self.acoustic = ort.InferenceSession(str(acoustic / "fastspeech2_mix.onnx"), opts)
        self.vocoder = ort.InferenceSession(str(vocoder / "hifigan_csmsc.onnx"), opts)
        entries = (acoustic / "phone_id_map.txt").read_text().splitlines()
        self.ids = {phone: int(index) for phone, index in (line.split() for line in entries)}
        self.pinyin = Pinyin(MandarinConverter())
        self.english = cmudict.dict()

    def mandarin_phones(self, text):
        text = cn2an.transform(text, "an2cn")
        options = dict(neutral_tone_with_five=True, errors=lambda chars: ["sp"])
        initials = self.pinyin.lazy_pinyin(text, style=Style.INITIALS, **options)
        finals = self.pinyin.lazy_pinyin(text, style=Style.FINALS_TONE3, **options)
        phones = []
        for initial, final in zip(initials, finals):
            if final == "sp":
                if phones and phones[-1] != "sp":
                    phones.append("sp")
                continue
            if final and final[-1] not in "12345":
                final += "5"
            if re.fullmatch(r"i[1-5]", final):
                if initial in ("z", "c", "s"):
                    final = "i" + final
                elif initial in ("zh", "ch", "sh", "r"):
                    final = "ii" + final
            phones.extend(p for p in (initial, final) if p)
        return phones

    def generate(self, text):
        phones = []
        for part in re.split(r"([A-Za-z]+(?:'[A-Za-z]+)*)", text):
            if not part:
                continue
            if not part[0].isascii() or not part[0].isalpha():
                phones.extend(self.mandarin_phones(part))
                continue
            words = [part] if part.lower() in self.english else re.split(r"(?<=[a-z])(?=[A-Z])", part)
            for word in words:
                entry = self.english.get(word.lower())
                if entry and not (word.isupper() and 1 < len(word) <= 4):
                    phones.extend(entry[0])
                else:
                    for letter in word.lower().replace("'", ""):
                        phones.extend(["EY1"] if letter == "a" else self.english[letter][0])
        if not phones or any(p not in self.ids for p in phones):
            return None
        ids = np.asarray([self.ids[p] for p in phones], dtype=np.int64)
        mel, = self.acoustic.run(None, {"text": ids, "spk_id": np.array([174], dtype=np.int64)})
        samples, = self.vocoder.run(None, {"logmel": mel})
        return samples.ravel()
