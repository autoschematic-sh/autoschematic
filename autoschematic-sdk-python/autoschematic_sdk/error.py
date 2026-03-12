class InvalidAddr(Exception):
    def __init__(self, addr):
        self.addr = addr
        self.msg = "Invalid addr"
        super().__init__(self.msg)

    def __str__(self):
        return f"{self.msg}: {self.addr}"
