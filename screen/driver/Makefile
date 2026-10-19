obj-m += panel-fl7707n.o
KDIR ?= /lib/modules/$(shell uname -r)/build

all:
	$(MAKE) -C $(KDIR) M=$(CURDIR) modules
	dtc -@ -I dts -O dtb -o fl7707n-dual.dtbo fl7707n-dual-overlay.dts

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean
	rm -f fl7707n-dual.dtbo
